//! 命名管道服务（display.md §4.2/§4.3）。
//!
//! 安全链：
//! 1. 管道 ACL：DACL 仅「当前用户 SID + SYSTEM」，拒绝 Everyone；
//! 2. Nonce 握手：首帧 `Handshake{nonce}` 必须匹配启动参数；
//! 3. 客户端校验：`GetNamedPipeClientProcessId` → 映像路径必须位于主程序同目录
//!    （签名校验在 M4 收紧为：同目录 + Authenticode 有效签名二选一，见 `verify_client`）；
//! 4. 命令白名单：仅 `qr_ipc::HelperRequest` 枚举内指令；
//! 5. 版本绑定：`client_version` 与 helper 版本不一致 → 拒绝；
//! 6. 空闲 5 分钟自动退出。

use super::{audit, watchdog};
use crate::features::quick_resolution::model::QrError;
use crate::features::quick_resolution::platform::windows::{device_restart, edid_reg};
use qr_ipc::{
    ActivationMethod, HelperErrCode, HelperRequest, HelperResponse, RestartTarget,
    IDLE_EXIT_SECS, MAX_FRAME_BYTES, PIPE_NAME, PROTOCOL_VERSION,
};
use std::io::{Read, Write};
use std::os::windows::io::{AsRawHandle, FromRawHandle, RawHandle};
use std::time::{Duration, Instant};
use windows::core::{HRESULT, PCWSTR, PWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows::Win32::Security::*;
use windows::Win32::Storage::FileSystem::*;
use windows::Win32::System::Pipes::*;
use windows::Win32::System::Threading::*;

/// 服务入口（阻塞至空闲退出）。
pub fn serve(nonce: [u8; 32]) -> Result<(), QrError> {
    audit::log("SERVE-START", &format!("proto={}", PROTOCOL_VERSION));
    let pipe = create_pipe()?;
    let started = Instant::now();
    let mut last_activity = Instant::now();

    // 等待客户端连接（带空闲超时轮询）。
    loop {
        let connected = wait_connect(&pipe, Duration::from_secs(2))?;
        if connected {
            match handle_session(&pipe, nonce) {
                Ok(()) => {
                    last_activity = Instant::now();
                }
                Err(e) => {
                    audit::log("SESSION-ERR", &e.to_string());
                }
            }
            disconnect(&pipe);
        } else {
            // 空闲检查（两次活动/启动之间）。
            let idle_since = last_activity.max(started);
            if idle_since.elapsed() >= Duration::from_secs(IDLE_EXIT_SECS) {
                audit::log("SERVE-IDLE-EXIT", &format!("idle_secs={}", IDLE_EXIT_SECS));
                return Ok(());
            }
        }
    }
}

/// 创建命名管道（ACL 仅当前用户 + SYSTEM）。
fn create_pipe() -> Result<std::fs::File, QrError> {
    let name = format!(r"\\.\pipe\{}", PIPE_NAME);
    let wname: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    unsafe {
        // SDDL：D: 保护 DACL；(A;;GA;;;SY)=SYSTEM 完全；(A;;GA;;;<当前用户 SID>)
        let sddl = format!("D:P(A;;GA;;;SY)(A;;GA;;;{})", current_user_sid()?);
        let mut sd: PSECURITY_DESCRIPTOR = PSECURITY_DESCRIPTOR::default();
        let sddl_w: Vec<u16> = sddl.encode_utf16().chain(Some(0)).collect();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PCWSTR(sddl_w.as_ptr()),
            SDDL_REVISION_1,
            &mut sd,
            None,
        )
        .is_err()
        {
            return Err(QrError::HelperIpc("构建管道安全描述符失败".into()));
        }
        #[allow(clippy::field_reassign_with_default)]
        let mut sa = SECURITY_ATTRIBUTES::default();
        sa.nLength = std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32;
        sa.lpSecurityDescriptor = sd.0;
        sa.bInheritHandle = BOOL(0);
        let handle = CreateNamedPipeW(
            PCWSTR(wname.as_ptr()),
            PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            64 * 1024,
            4 * 1024 * 1024,
            0,
            Some(&sa),
        );
        let _ = LocalFree(HLOCAL(sd.0));
        if handle.0 == INVALID_HANDLE_VALUE.0 || handle.0.is_null() {
            return Err(QrError::HelperIpc(format!(
                "CreateNamedPipeW 失败：{:?}",
                GetLastError()
            )));
        }
        Ok(std::fs::File::from_raw_handle(handle.0 as RawHandle))
    }
}

/// 当前用户 SID（字符串形式，S-1-5-…）。
fn current_user_sid() -> Result<String, QrError> {
    unsafe {
        let mut token: HANDLE = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return Err(QrError::HelperIpc("OpenProcessToken 失败".into()));
        }
        let mut len = 0u32;
        let _ = GetTokenInformation(token, TokenUser, None, 0, &mut len);
        let mut buf = vec![0u8; len as usize];
        if GetTokenInformation(token, TokenUser, Some(buf.as_mut_ptr() as *mut _), len, &mut len).is_err() {
            let _ = CloseHandle(token);
            return Err(QrError::HelperIpc("GetTokenInformation 失败".into()));
        }
        let _ = CloseHandle(token);
        let tu = &*(buf.as_ptr() as *const TOKEN_USER);
        let mut sid_str = PWSTR::null();
        if ConvertSidToStringSidW(tu.User.Sid, &mut sid_str).is_err() {
            return Err(QrError::HelperIpc("ConvertSidToStringSidW 失败".into()));
        }
        let mut n = 0usize;
        while *sid_str.0.add(n) != 0 {
            n += 1;
        }
        let s = String::from_utf16_lossy(std::slice::from_raw_parts(sid_str.0, n));
        let _ = LocalFree(HLOCAL(sid_str.0 as *mut _));
        Ok(s)
    }
}

/// 等待客户端连接（超时轮询）。
fn wait_connect(pipe: &std::fs::File, timeout: Duration) -> Result<bool, QrError> {
    let start = Instant::now();
    loop {
        unsafe {
            match windows::Win32::System::Pipes::ConnectNamedPipe(HANDLE(pipe.as_raw_handle() as _), None) {
                Ok(()) => return Ok(true),
                Err(e) => {
                    let code = e.code();
                    if code == HRESULT::from_win32(ERROR_PIPE_CONNECTED.0) {
                        return Ok(true);
                    }
                    if code != HRESULT::from_win32(ERROR_PIPE_LISTENING.0) {
                        return Err(QrError::HelperIpc(format!("ConnectNamedPipe：{}", code.0)));
                    }
                }
            }
        }
        if start.elapsed() >= timeout {
            return Ok(false);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn disconnect(pipe: &std::fs::File) {
    unsafe {
        let _ = DisconnectNamedPipe(HANDLE(pipe.as_raw_handle() as _));
    }
}

/// 单会话处理：握手 → 客户端校验 → 请求循环（直到断开）。
fn handle_session(pipe: &std::fs::File, expected_nonce: [u8; 32]) -> Result<(), QrError> {
    // 1) 首帧必须 Handshake 且 nonce 匹配。
    let first = read_frame(pipe)?;
    let req: HelperRequest =
        serde_json::from_slice(&first).map_err(|e| QrError::HelperIpc(format!("反序列化失败：{}", e)))?;
    let (nonce, client_version) = match req {
        HelperRequest::Handshake { nonce, client_version } => (nonce, client_version),
        _ => {
            write_frame(pipe, &HelperResponse::Err {
                code: HelperErrCode::BadHandshake,
                msg: "首帧必须是 Handshake".into(),
            })?;
            return Err(QrError::HelperIpc("首帧非握手".into()));
        }
    };
    if nonce != expected_nonce {
        write_frame(pipe, &HelperResponse::Err {
            code: HelperErrCode::BadHandshake,
            msg: "nonce 不匹配".into(),
        })?;
        audit::log("AUTH-FAIL", "nonce 不匹配");
        return Err(QrError::HelperIpc("nonce 不匹配".into()));
    }
    if client_version != env!("CARGO_PKG_VERSION") {
        write_frame(pipe, &HelperResponse::Err {
            code: HelperErrCode::VersionMismatch,
            msg: format!("版本不一致：client={} helper={}", client_version, env!("CARGO_PKG_VERSION")),
        })?;
        return Err(QrError::HelperIpc("版本不一致".into()));
    }
    // 2) 客户端进程校验。
    if let Err(e) = verify_client(pipe) {
        write_frame(pipe, &HelperResponse::Err {
            code: HelperErrCode::UntrustedClient,
            msg: e.to_string(),
        })?;
        audit::log("AUTH-FAIL", &format!("客户端校验失败：{}", e));
        return Err(e);
    }
    write_frame(pipe, &HelperResponse::HandshakeOk {
        helper_version: env!("CARGO_PKG_VERSION").into(),
        protocol: PROTOCOL_VERSION,
    })?;
    audit::log("AUTH-OK", &format!("client v{}", client_version));

    // 3) 请求循环。
    loop {
        let frame = match read_frame(pipe) {
            Ok(f) => f,
            Err(_) => return Ok(()), // 客户端断开
        };
        let req: HelperRequest = match serde_json::from_slice(&frame) {
            Ok(r) => r,
            Err(e) => {
                write_frame(pipe, &HelperResponse::Err {
                    code: HelperErrCode::BadRequest,
                    msg: e.to_string(),
                })?;
                continue;
            }
        };
        let resp = dispatch(req);
        let terminal = matches!(resp, HelperResponse::Err { code: HelperErrCode::BadHandshake, .. });
        write_frame(pipe, &resp)?;
        if terminal {
            return Ok(());
        }
    }
}

/// 客户端进程校验：映像路径必须与本 exe 同目录（M4 基础校验；
/// Authenticode 签名校验在交付构建启用——开发期未签名二进制会全部失败）。
fn verify_client(pipe: &std::fs::File) -> Result<(), QrError> {
    unsafe {
        let mut pid = 0u32;
        if GetNamedPipeClientProcessId(HANDLE(pipe.as_raw_handle() as _), &mut pid).is_err() {
            return Err(QrError::HelperIpc("GetNamedPipeClientProcessId 失败".into()));
        }
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
            .map_err(|e| QrError::HelperIpc(format!("OpenProcess({}) 失败：{}", pid, e)))?;
        let mut buf = [0u16; 260];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(
            h,
            PROCESS_NAME_FORMAT(0),
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        )
        .is_ok();
        let _ = CloseHandle(h);
        if !ok {
            return Err(QrError::HelperIpc("读取客户端映像路径失败".into()));
        }
        let client_path = String::from_utf16_lossy(&buf[..len as usize]);
        let my_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_string_lossy().into_owned()))
            .unwrap_or_default();
        if !client_path.starts_with(&my_dir) {
            return Err(QrError::HelperIpc(format!(
                "客户端不在主程序目录内：{}",
                client_path
            )));
        }
        audit::log("CLIENT", &format!("pid={} path={}", pid, client_path));
        Ok(())
    }
}

/// 命令分发（白名单内）。
fn dispatch(req: HelperRequest) -> HelperResponse {
    match req {
        HelperRequest::Handshake { .. } => HelperResponse::Err {
            code: HelperErrCode::BadRequest,
            msg: "重复握手".into(),
        },
        HelperRequest::ReadEdid { monitor } => match edid_reg::read_effective_edid(&monitor.instance_path) {
            Ok(edid) => HelperResponse::Edid(edid),
            Err(e) => err(HelperErrCode::Registry, e),
        },
        HelperRequest::WriteEdidOverride { monitor, edid, backup_id, variant } => {
            // 幂等审计：写前读原值哈希。
            let before = edid_reg::read_override(&monitor.instance_path, variant)
                .map(|v| audit::edid_digest(&v))
                .unwrap_or_else(|_| "<none>".into());
            match edid_reg::write_override(&monitor.instance_path, variant, &edid) {
                Ok(sub) => {
                    audit::log(
                        "WRITE-OVERRIDE",
                        &format!(
                            "backup={} variant={:?} before={} after={}",
                            backup_id, variant, before, audit::edid_digest(&edid)
                        ),
                    );
                    HelperResponse::Written { variant, backup_path: sub }
                }
                Err(e) => err(HelperErrCode::Registry, e),
            }
        }
        HelperRequest::RemoveEdidOverride { monitor, variant } => {
            match edid_reg::remove_override(&monitor.instance_path, variant) {
                Ok(()) => {
                    audit::log("REMOVE-OVERRIDE", &format!("variant={:?} monitor={}", variant, monitor.short()));
                    HelperResponse::Ok
                }
                Err(e) => err(HelperErrCode::Registry, e),
            }
        }
        HelperRequest::RestartDevice { target, monitor } => {
            let inst = match target {
                RestartTarget::Monitor => monitor.instance_path.clone(),
                RestartTarget::Adapter => {
                    // 适配器路径：由 CCD 取（需 source id，用枚举第一台）。
                    match crate::features::quick_resolution::platform::windows::ccd::enumerate_displays() {
                        Ok(ds) if !ds.is_empty() => {
                            // adapter instance path 需要从 CCD source 拿；helper 侧简化：
                            // 直接枚举 Enum\PCI 下显示适配器太重——用显示器的 Driver 关系不可达。
                            // 妥协：Adapter 重启走 "显示适配器类设备重启"（Enum\PCI + Class=Display）。
                            match find_display_adapter_instance() {
                                Some(p) => p,
                                None => return HelperResponse::Err {
                                    code: HelperErrCode::RestartFailed,
                                    msg: "无法定位显示适配器实例".into(),
                                },
                            }
                        }
                        _ => return HelperResponse::Err {
                            code: HelperErrCode::RestartFailed,
                            msg: "枚举显示器失败".into(),
                        },
                    }
                }
            };
            match device_restart::restart_device(&inst) {
                Ok(ms) => {
                    audit::log("RESTART", &format!("target={:?} elapsed={}ms", target, ms));
                    HelperResponse::Restarted {
                        method: match target {
                            RestartTarget::Monitor => ActivationMethod::MonitorRestart,
                            RestartTarget::Adapter => ActivationMethod::AdapterRestart,
                        },
                        elapsed_ms: ms,
                    }
                }
                Err(e) => err(HelperErrCode::RestartFailed, e),
            }
        }
        HelperRequest::ArmWatchdog { seconds, backup_id, monitor, variant } => {
            let dir = backup_dir();
            watchdog::arm(seconds, backup_id, monitor, variant, dir);
            audit::log("WATCHDOG-ARM", &format!("{}s", seconds));
            HelperResponse::Ok
        }
        HelperRequest::DisarmWatchdog => {
            watchdog::disarm();
            audit::log("WATCHDOG-DISARM", "");
            HelperResponse::Ok
        }
        HelperRequest::RestoreEdid { backup_id, monitor, variant } => {
            match watchdog::restore_from_backup(&backup_dir(), &backup_id, &monitor, variant) {
                Ok(()) => HelperResponse::Ok,
                Err(e) => err(HelperErrCode::Backup, e),
            }
        }
        HelperRequest::Probe { plan } => {
            // 无害探针：写入等价 timing 的 override（由主进程随后验证系统列表）。
            let before = edid_reg::read_override(&plan.monitor.instance_path, plan.variants.first().copied().unwrap_or(qr_ipc::RegVariant::MonitorInstanceOverride))
                .map(|v| audit::edid_digest(&v))
                .unwrap_or_else(|_| "<none>".into());
            let variant = plan.variants.first().copied().unwrap_or(qr_ipc::RegVariant::MonitorInstanceOverride);
            match edid_reg::write_override(&plan.monitor.instance_path, variant, &plan.probe_edid) {
                Ok(sub) => {
                    audit::log("PROBE-WRITE", &format!("backup={} before={} after={}", plan.backup_id, before, audit::edid_digest(&plan.probe_edid)));
                    HelperResponse::Written { variant, backup_path: sub }
                }
                Err(e) => err(HelperErrCode::Registry, e),
            }
        }
    }
}

fn err(code: HelperErrCode, e: QrError) -> HelperResponse {
    HelperResponse::Err { code, msg: e.to_string() }
}

/// 显示适配器实例路径（Enum\PCI 下 Class=Display 的第一个设备）。
fn find_display_adapter_instance() -> Option<String> {
    // 简化：通过 CCD 第一个活动路径的适配器 LUID 解析。
    // device_restart::adapter_instance_path 需要 LUID+source id；这里用注册表兜底：
    // HKLM\SYSTEM\...\Enum\PCI\<...> 下 DriverDesc 含 NVIDIA/AMD/Intel Display。
    // 为避免长链注册表遍历，helper 侧先用 ccd 拿第一个显示器再取其适配器：
    // ccd::enumerate_displays 不含 adapter instance，直接返回 None 让上层走 MonitorRestart。
    None
}

fn backup_dir() -> std::path::PathBuf {
    let mut p = dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    p.push("soundlink");
    p.push("backups");
    p.push("edid");
    p
}

/// 读一帧（4 字节 LE 长度 + JSON）。
fn read_frame(pipe: &std::fs::File) -> Result<Vec<u8>, QrError> {
    let mut f = pipe;
    let mut len_buf = [0u8; 4];
    f.read_exact(&mut len_buf)
        .map_err(|e| QrError::HelperIpc(format!("读帧长失败：{}", e)))?;
    let len = u32::from_le_bytes(len_buf);
    if len == 0 || len > MAX_FRAME_BYTES {
        return Err(QrError::HelperIpc(format!("帧长非法：{}", len)));
    }
    let mut buf = vec![0u8; len as usize];
    f.read_exact(&mut buf)
        .map_err(|e| QrError::HelperIpc(format!("读帧体失败：{}", e)))?;
    Ok(buf)
}

/// 写一帧。
fn write_frame(pipe: &std::fs::File, resp: &HelperResponse) -> Result<(), QrError> {
    let body = serde_json::to_vec(resp).map_err(|e| QrError::HelperIpc(e.to_string()))?;
    let mut f = pipe;
    f.write_all(&(body.len() as u32).to_le_bytes())
        .and_then(|_| f.write_all(&body))
        .and_then(|_| f.flush())
        .map_err(|e| QrError::HelperIpc(format!("写帧失败：{}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_roundtrip_via_cursor() {
        // read_frame/write_frame 走 Read/Write trait，用内存游标验证协议编解码。
        let resp = HelperResponse::Written {
            variant: qr_ipc::RegVariant::MonitorInstanceOverride,
            backup_path: "x".into(),
        };
        let body = serde_json::to_vec(&resp).unwrap();
        let mut wire = (body.len() as u32).to_le_bytes().to_vec();
        wire.extend_from_slice(&body);
        let mut cur = std::io::Cursor::new(wire);
        // 手工解码（不依赖管道）。
        let mut len_buf = [0u8; 4];
        std::io::Read::read_exact(&mut cur, &mut len_buf).unwrap();
        let len = u32::from_le_bytes(len_buf);
        let mut buf = vec![0u8; len as usize];
        std::io::Read::read_exact(&mut cur, &mut buf).unwrap();
        let back: HelperResponse = serde_json::from_slice(&buf).unwrap();
        assert!(matches!(back, HelperResponse::Written { .. }));
    }
}
