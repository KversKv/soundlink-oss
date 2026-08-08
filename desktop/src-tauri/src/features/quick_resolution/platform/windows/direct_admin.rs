//! 主进程管理员直写路径（QR-1 追加）。
//!
//! 主程序以管理员身份运行时，EDID override 写入与设备重启无需经 helper
//! 计划任务转发，本进程直接执行。**看门狗例外**：它是「独立进程盯着主进程」，
//! 主进程崩溃时由它还原 EDID——该职责必须留在 helper，不能因直写而省略
//! （否则主进程崩溃将无任何自动还原，黑屏保险失效）。
//!
//! 仅 [`crate::features::quick_resolution::provisioner`] 在
//! [`is_elevated`] 为真时调用本模块；普通权限下不会触达。

use crate::features::quick_resolution::model::QrError;
use crate::features::quick_resolution::platform::windows::{device_restart, edid_reg};
use qr_ipc::{MonitorKey, RegVariant};
use windows::Win32::Foundation::*;
use windows::Win32::Security::*;
use windows::Win32::System::Threading::*;

/// 当前进程是否以管理员（提升的 Administrators）运行。
pub fn is_elevated() -> bool {
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION::default();
        let mut len = std::mem::size_of::<TOKEN_ELEVATION>() as u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            len,
            &mut len,
        )
        .is_ok();
        let _ = CloseHandle(token);
        ok && elevation.TokenIsElevated != 0
    }
}

/// 写 EDID override（本进程直接写 HKLM）。
pub fn write_override(monitor: &MonitorKey, variant: RegVariant, edid: &[u8]) -> Result<String, QrError> {
    edid_reg::write_override(&monitor.instance_path, variant, edid)
}

/// 移除 EDID override（本进程直接删 HKLM 值）。
pub fn remove_override(monitor: &MonitorKey, variant: RegVariant) -> Result<(), QrError> {
    edid_reg::remove_override(&monitor.instance_path, variant)
}

/// 重启显示器（禁用→启用）。返回耗时毫秒。
pub fn restart_monitor(monitor: &MonitorKey) -> Result<u64, QrError> {
    device_restart::restart_device(&monitor.instance_path)
}

/// 重启显示适配器（第一块活动路径对应的适配器）。返回耗时毫秒。
pub fn restart_adapter() -> Result<u64, QrError> {
    let (adapter, source_id) = super::ccd::first_active_adapter()
        .ok_or_else(|| QrError::DisplayNotFound("无活动显示路径".into()))?;
    let instance = device_restart::adapter_instance_path(adapter, source_id)?;
    device_restart::restart_device(&instance)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_elevated_does_not_panic() {
        // CI/开发机多为非提权，仅断言可调用返回布尔。
        let _ = is_elevated();
    }
}
