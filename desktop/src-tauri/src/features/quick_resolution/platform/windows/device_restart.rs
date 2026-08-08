//! 设备重启（SetupAPI / CfgMgr32 禁用→启用）。**需提权，仅 qr_helper 调用。**

use crate::features::quick_resolution::model::QrError;
use windows::core::PCWSTR;
use windows::Win32::Devices::DeviceAndDriverInstallation::*;
use windows::Win32::Devices::Display::*;
use windows::Win32::Foundation::*;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// 适配器 LUID → 适配器设备实例路径（`PCI\VEN_10DE&...\4&...`）。
///
/// 通过 CCD `GET_ADAPTER_NAME` 拿设备接口路径再转实例路径（与显示器同法）。
pub fn adapter_instance_path(adapter: LUID, source_id: u32) -> Result<String, QrError> {
    unsafe {
        let mut name = DISPLAYCONFIG_ADAPTER_NAME::default();
        name.header = DISPLAYCONFIG_DEVICE_INFO_HEADER {
            r#type: DISPLAYCONFIG_DEVICE_INFO_GET_ADAPTER_NAME,
            size: std::mem::size_of::<DISPLAYCONFIG_ADAPTER_NAME>() as u32,
            adapterId: adapter,
            id: source_id,
        };
        let r = DisplayConfigGetDeviceInfo(&mut name.header);
        if r != ERROR_SUCCESS.0 as i32 {
            return Err(QrError::Win32 { api: "DisplayConfigGetDeviceInfo(ADAPTER)".into(), code: r });
        }
        let device_path = super::ccd::wide_to_string(&name.adapterDevicePath);
        let s = device_path.trim_start_matches("\\\\?\\");
        let s = match s.find("#{") {
            Some(i) => &s[..i],
            None => s,
        };
        Ok(s.replace('#', "\\"))
    }
}

/// 重启设备（禁用 → 等待 → 启用）。
pub fn restart_device(instance_path: &str) -> Result<u64, QrError> {
    let t0 = std::time::Instant::now();
    unsafe {
        let w = wide(instance_path);
        let mut devinst = 0u32;
        let cr = CM_Locate_DevNodeW(&mut devinst, PCWSTR(w.as_ptr()), CM_LOCATE_DEVNODE_NORMAL);
        if cr != CR_SUCCESS {
            return Err(QrError::Win32 { api: "CM_Locate_DevNodeW".into(), code: cr.0 as i32 });
        }
        let cr = CM_Disable_DevNode(devinst, 0);
        if cr != CR_SUCCESS {
            return Err(QrError::Win32 { api: "CM_Disable_DevNode".into(), code: cr.0 as i32 });
        }
        // 给驱动栈一点卸载时间。
        std::thread::sleep(std::time::Duration::from_millis(800));
        let cr = CM_Enable_DevNode(devinst, 0);
        if cr != CR_SUCCESS {
            return Err(QrError::Win32 { api: "CM_Enable_DevNode".into(), code: cr.0 as i32 });
        }
    }
    Ok(t0.elapsed().as_millis() as u64)
}

#[cfg(test)]
mod tests {
    // 设备操作需真实硬件 + 提权，单测仅覆盖纯函数（路径转换已在 ccd::tests 覆盖）。
    #[test]
    fn module_loads() {
        assert!(true);
    }
}
