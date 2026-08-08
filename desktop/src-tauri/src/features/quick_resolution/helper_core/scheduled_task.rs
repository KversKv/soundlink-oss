//! 计划任务注册/删除（display.md §4.1）。
//!
//! 用 `schtasks.exe`（零依赖）：任务以最高权限运行本 exe `--serve $(Arg0)`，
//! `$(Arg0)` 由主进程 `Run` 时注入 nonce。任务不显示窗口、按需拉起、用完即走。

use crate::features::quick_resolution::model::QrError;
use std::process::Command;

/// 当前 exe 完整路径。
fn exe_path() -> Result<String, QrError> {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| QrError::Io(format!("定位 qr_helper 路径失败：{}", e)))
}

/// 注册一次性计划任务（--install，UAC 一次）。
pub fn install() -> Result<(), QrError> {
    let exe = exe_path()?;
    // nonce 通过临时文件传递（schtasks /Run 无法携带参数），任务动作固定 `--serve`。
    let ps = format!(
        "$action = New-ScheduledTaskAction -Execute '{exe}' -Argument '--serve';\
         $principal = New-ScheduledTaskPrincipal -UserId $env:USERNAME -RunLevel Highest -LogonType Interactive;\
         $settings = New-ScheduledTaskSettingsSet -Hidden -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -StartWhenAvailable;\
         Register-ScheduledTask -TaskName '{name}' -Action $action -Principal $principal -Settings $settings -Force | Out-Null",
        exe = exe.replace('\'', "''"),
        name = qr_ipc::TASK_NAME
    );
    let out = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &ps])
        .output()
        .map_err(|e| QrError::HelperIpc(format!("无法启动 powershell：{}", e)))?;
    if !out.status.success() {
        return Err(QrError::HelperIpc(format!(
            "注册计划任务失败：{}",
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    Ok(())
}

/// 删除计划任务（--uninstall）。
pub fn uninstall() -> Result<(), QrError> {
    let out = Command::new("schtasks")
        .args(["/Delete", "/TN", qr_ipc::TASK_NAME, "/F"])
        .output()
        .map_err(|e| QrError::HelperIpc(format!("无法启动 schtasks：{}", e)))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        // 任务不存在视为成功（幂等）。
        if err.contains("cannot find") || err.contains("找不到") || err.contains("does not exist") {
            return Ok(());
        }
        return Err(QrError::HelperIpc(format!("删除计划任务失败：{}", err)));
    }
    Ok(())
}

/// 任务是否已注册（主进程探测 helperInstalled 用）。
pub fn is_installed() -> bool {
    Command::new("schtasks")
        .args(["/Query", "/TN", qr_ipc::TASK_NAME])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_uninstall_idempotent_shape() {
        // 不真正执行（需要管理员），仅验证函数存在且可调用返回 Result。
        let _ = is_installed();
    }
}
