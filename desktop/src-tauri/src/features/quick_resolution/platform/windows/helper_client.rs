//! 主进程侧 helper 客户端（display.md §4.1/§4.3）。
//!
//! 会话流程：生成 nonce → `schtasks /Run` 拉起 helper（免 UAC）→
//! 连命名管道 → 首帧 Handshake{nonce} → 请求/响应循环。
//! 失败降级：计划任务未注册/被杀软拦截 → `HelperNotInstalled`（保留每次 UAC 侧车路径为 fallback，M4 未实现 sidecar）。

use crate::features::quick_resolution::model::QrError;
use qr_ipc::{
    HelperRequest, HelperResponse, HelperErrCode, PIPE_NAME, PROTOCOL_VERSION,
};
use std::io::{Read, Write};
use std::os::windows::io::{FromRawHandle, RawHandle};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::*;
use windows::Win32::Storage::FileSystem::*;
use windows::Win32::System::Pipes::*;

/// 一次 helper 会话（用完即关；helper 空闲 5 分钟自动退出）。
pub struct HelperSession {
    pipe: std::fs::File,
}

impl HelperSession {
    /// 建立会话：拉起计划任务 + 连管道 + 握手。
    pub fn connect() -> Result<Self, QrError> {
        // 1) 生成 nonce 并拉起计划任务。
        let nonce = rand::random::<[u8; 32]>();
        let nonce_hex: String = nonce.iter().map(|b| format!("{:02x}", b)).collect();
        run_scheduled_task(&nonce_hex)?;

        // 2) 连管道（轮询等待 helper 起服务）。
        let pipe = connect_pipe(Duration::from_secs(8))?;
        let mut s = Self { pipe };

        // 3) 握手。
        let resp = s.request(&HelperRequest::Handshake {
            nonce,
            client_version: env!("CARGO_PKG_VERSION").into(),
        })?;
        match resp {
            HelperResponse::HandshakeOk { protocol, .. } if protocol == PROTOCOL_VERSION => Ok(s),
            HelperResponse::HandshakeOk { .. } => Err(QrError::HelperIpc("协议版本不匹配".into())),
            HelperResponse::Err { code, msg } => Err(QrError::HelperIpc(format!("{:?}: {}", code, msg))),
            _ => Err(QrError::HelperIpc("握手响应异常".into())),
        }
    }

    /// 发一帧请求，收一帧响应。
    pub fn request(&mut self, req: &HelperRequest) -> Result<HelperResponse, QrError> {
        let body = serde_json::to_vec(req).map_err(|e| QrError::HelperIpc(e.to_string()))?;
        write_all(&mut self.pipe, &(body.len() as u32).to_le_bytes())?;
        write_all(&mut self.pipe, &body)?;
        self.pipe.flush().map_err(|e| QrError::HelperIpc(e.to_string()))?;

        let mut len_buf = [0u8; 4];
        self.pipe
            .read_exact(&mut len_buf)
            .map_err(|e| QrError::HelperIpc(format!("读帧长失败：{}", e)))?;
        let len = u32::from_le_bytes(len_buf);
        if len == 0 || len > qr_ipc::MAX_FRAME_BYTES {
            return Err(QrError::HelperIpc(format!("帧长非法：{}", len)));
        }
        let mut buf = vec![0u8; len as usize];
        self.pipe
            .read_exact(&mut buf)
            .map_err(|e| QrError::HelperIpc(format!("读帧体失败：{}", e)))?;
        serde_json::from_slice(&buf).map_err(|e| QrError::HelperIpc(e.to_string()))
    }

    /// 请求 + 期望 Ok/特定响应，Err 响应转 QrError。
    pub fn call(&mut self, req: &HelperRequest) -> Result<HelperResponse, QrError> {
        match self.request(req)? {
            HelperResponse::Err { code, msg } => Err(map_helper_err(code, &msg)),
            ok => Ok(ok),
        }
    }
}

fn map_helper_err(code: HelperErrCode, msg: &str) -> QrError {
    match code {
        HelperErrCode::VersionMismatch => QrError::HelperIpc(format!("版本不一致：{}", msg)),
        HelperErrCode::UntrustedClient => QrError::HelperIpc(format!("客户端校验失败：{}", msg)),
        _ => QrError::HelperIpc(msg.to_string()),
    }
}

/// `schtasks /Run` 拉起任务（参数经 /TN 任务的 $(Arg0) 注入需 Run 带参——schtasks 不支持
/// Run 传参；因此 nonce 通过临时文件传递给 helper：`%APPDATA%/soundlink/qr_nonce.tmp`）。
fn run_scheduled_task(nonce_hex: &str) -> Result<(), QrError> {
    // schtasks /Run 无法携带参数；任务动作固定 `--serve`，nonce 走临时文件（仅当前用户可读目录）。
    let mut p = dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    p.push("soundlink");
    let _ = std::fs::create_dir_all(&p);
    p.push("qr_nonce.tmp");
    std::fs::write(&p, nonce_hex).map_err(|e| QrError::HelperIpc(format!("写 nonce 失败：{}", e)))?;

    let out = std::process::Command::new("schtasks")
        .args(["/Run", "/TN", qr_ipc::TASK_NAME])
        .output()
        .map_err(|e| QrError::HelperIpc(format!("启动 schtasks 失败：{}", e)))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        if err.contains("cannot find") || err.contains("找不到") || err.contains("does not exist") {
            return Err(QrError::HelperNotInstalled);
        }
        return Err(QrError::HelperIpc(format!("计划任务启动失败：{}", err)));
    }
    Ok(())
}

/// helper 侧读取 nonce 的文件路径（pipe_server 启动时读一次后删除）。
pub fn nonce_file_path() -> std::path::PathBuf {
    let mut p = dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    p.push("soundlink");
    p.push("qr_nonce.tmp");
    p
}

/// 连接命名管道（等待 helper 起来）。
fn connect_pipe(timeout: Duration) -> Result<std::fs::File, QrError> {
    let name = format!(r"\\.\pipe\{}", PIPE_NAME);
    let wname: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    let start = Instant::now();
    loop {
        unsafe {
            let handle = CreateFileW(
                windows::core::PCWSTR(wname.as_ptr()),
                GENERIC_READ.0 | GENERIC_WRITE.0,
                FILE_SHARE_MODE(0),
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(0),
                None,
            );
            match handle {
                Ok(h) if h.0 != INVALID_HANDLE_VALUE.0 && !h.0.is_null() => {
                    return Ok(std::fs::File::from_raw_handle(h.0 as RawHandle));
                }
                _ => {
                    let err = GetLastError();
                    if err != ERROR_PIPE_BUSY && err != ERROR_FILE_NOT_FOUND {
                        return Err(QrError::HelperIpc(format!("连接管道失败：{:?}", err)));
                    }
                    // 等管道可用。
                    let _ = WaitNamedPipeW(windows::core::PCWSTR(wname.as_ptr()), 500);
                }
            }
        }
        if start.elapsed() >= timeout {
            return Err(QrError::HelperIpc("连接 helper 超时".into()));
        }
        std::thread::sleep(Duration::from_millis(120));
    }
}

fn write_all(f: &mut std::fs::File, data: &[u8]) -> Result<(), QrError> {
    f.write_all(data).map_err(|e| QrError::HelperIpc(format!("写管道失败：{}", e)))
}

/// helper 是否已安装（计划任务存在）。
pub fn helper_installed() -> bool {
    std::process::Command::new("schtasks")
        .args(["/Query", "/TN", qr_ipc::TASK_NAME])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// 触发一次性 UAC 安装（ShellExecute runas qr_helper --install）。
pub fn install_helper() -> Result<(), QrError> {
    let exe = std::env::current_exe()
        .map_err(|e| QrError::Io(format!("定位主程序失败：{}", e)))?;
    // qr_helper.exe 与主程序同目录。
    let helper = exe
        .parent()
        .map(|d| d.join("qr_helper.exe"))
        .ok_or_else(|| QrError::HelperIpc("无法定位 qr_helper.exe".into()))?;
    if !helper.exists() {
        return Err(QrError::HelperIpc("qr_helper.exe 不存在（需随包发布）".into()));
    }
    let op: Vec<u16> = "runas\0".encode_utf16().collect();
    let file: Vec<u16> = helper.to_string_lossy().encode_utf16().chain(Some(0)).collect();
    let args: Vec<u16> = "--install\0".encode_utf16().collect();
    unsafe {
        let r = windows::Win32::UI::Shell::ShellExecuteW(
            None,
            windows::core::PCWSTR(op.as_ptr()),
            windows::core::PCWSTR(file.as_ptr()),
            windows::core::PCWSTR(args.as_ptr()),
            None,
            windows::Win32::UI::WindowsAndMessaging::SW_HIDE,
        );
        // ShellExecute 返回值 >32 为成功。
        if (r.0 as isize) <= 32 {
            return Err(QrError::ElevationDenied);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonce_path_under_config() {
        let p = nonce_file_path();
        assert!(p.to_string_lossy().ends_with("qr_nonce.tmp"));
    }
}
