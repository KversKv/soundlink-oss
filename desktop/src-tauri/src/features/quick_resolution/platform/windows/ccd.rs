//! CCD（Connecting and Configuring Displays）枚举：显示器编号/标识/主键。
//!
//! 三层标识（display.md §8.1）：
//! - 展示层编号 = source id 排序 1..N（与 Windows 显示设置一致）；
//! - 稳定层 `MonitorKey` = 设备实例路径 + 原生 EDID 哈希（重启/换口不丢）；
//! - 系统层 `\\.\DISPLAYn` 每次运行重新解析，绝不持久化。

use super::{edid_reg, gdi};
use crate::features::quick_resolution::model::{DisplayInfo, DscState, QrError};
use qr_ipc::MonitorKey;
use sha2::Digest;
use windows::Win32::Devices::Display::*;
use windows::Win32::Foundation::*;

/// 枚举活动显示器。
pub fn enumerate_displays() -> Result<Vec<DisplayInfo>, QrError> {
    let paths = query_active_paths()?;
    let mut rows: Vec<(u32, DisplayInfo)> = Vec::new();
    for p in &paths {
        let source_id = p.sourceInfo.id;
        let adapter = p.sourceInfo.adapterId;
        let gdi_name = match source_gdi_name(adapter, source_id) {
            Some(n) => n,
            None => continue,
        };
        let (friendly, instance_path) = match target_info(adapter, p.targetInfo.id) {
            Some(v) => v,
            None => (String::new(), String::new()),
        };
        if instance_path.is_empty() {
            continue;
        }
        // 原生 EDID（key 用，保证 override 注入后 key 稳定）。
        let native_edid = edid_reg::read_native_edid(&instance_path).ok();
        let edid_hash = match &native_edid {
            Some(edid) => hex_prefix(&sha2::Sha256::digest(edid), 16),
            None => hex_prefix(&sha2::Sha256::digest(instance_path.as_bytes()), 16),
        };
        let current = gdi::current_mode(&gdi_name).ok();
        let is_primary = gdi::is_primary(&gdi_name);
        let max_pixel_clock_khz = native_edid
            .as_deref()
            .and_then(|e| qr_edid::parse::parse(e).ok())
            .and_then(|info| info.max_pixel_clock_khz);
        rows.push((
            source_id,
            DisplayInfo {
                index: 0, // 排序后回填
                key: MonitorKey { instance_path, edid_hash },
                gdi_name,
                friendly_name: if friendly.is_empty() { "未知显示器".into() } else { friendly },
                is_primary,
                current,
                link: None,
                dsc: DscState::Unknown { reason: "未检测".into(), debug: Vec::new() },
                max_pixel_clock_khz,
            },
        ));
    }
    // 编号：source id 升序 → 1..N。
    rows.sort_by_key(|(sid, _)| *sid);
    for (i, (_, d)) in rows.iter_mut().enumerate() {
        d.index = (i + 1) as u32;
    }
    Ok(rows.into_iter().map(|(_, d)| d).collect())
}

/// 活动显示路径。
fn query_active_paths() -> Result<Vec<DISPLAYCONFIG_PATH_INFO>, QrError> {
    unsafe {
        let mut num_paths = 0u32;
        let mut num_modes = 0u32;
        let flags = QDC_ONLY_ACTIVE_PATHS;
        let r = GetDisplayConfigBufferSizes(flags, &mut num_paths, &mut num_modes);
        if r != ERROR_SUCCESS {
            return Err(QrError::Win32 { api: "GetDisplayConfigBufferSizes".into(), code: r.0 as i32 });
        }
        if num_paths == 0 {
            return Ok(Vec::new());
        }
        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); num_paths as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); num_modes as usize];
        let r = QueryDisplayConfig(
            flags,
            &mut num_paths,
            paths.as_mut_ptr(),
            &mut num_modes,
            modes.as_mut_ptr(),
            None,
        );
        if r != ERROR_SUCCESS {
            return Err(QrError::Win32 { api: "QueryDisplayConfig".into(), code: r.0 as i32 });
        }
        paths.truncate(num_paths as usize);
        Ok(paths)
    }
}

/// source → GDI 设备名（`\\.\DISPLAY1`）。
fn source_gdi_name(adapter: LUID, source_id: u32) -> Option<String> {
    unsafe {
        let mut name = DISPLAYCONFIG_SOURCE_DEVICE_NAME::default();
        name.header = DISPLAYCONFIG_DEVICE_INFO_HEADER {
            r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
            size: std::mem::size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32,
            adapterId: adapter,
            id: source_id,
        };
        let r = DisplayConfigGetDeviceInfo(&mut name.header);
        if r != ERROR_SUCCESS.0 as i32 {
            return None;
        }
        Some(wide_to_string(&name.viewGdiDeviceName))
    }
}

/// target → (friendly name, 设备实例路径)。
fn target_info(adapter: LUID, target_id: u32) -> Option<(String, String)> {
    unsafe {
        let mut name = DISPLAYCONFIG_TARGET_DEVICE_NAME::default();
        name.header = DISPLAYCONFIG_DEVICE_INFO_HEADER {
            r#type: DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
            size: std::mem::size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32,
            adapterId: adapter,
            id: target_id,
        };
        let r = DisplayConfigGetDeviceInfo(&mut name.header);
        if r != ERROR_SUCCESS.0 as i32 {
            return None;
        }
        let friendly = wide_to_string(&name.monitorFriendlyDeviceName);
        let device_path = wide_to_string(&name.monitorDevicePath);
        Some((friendly, device_path_to_instance(&device_path)))
    }
}

/// `\\?\DISPLAY#SHP155B#5&2f3a&0&UID261#{guid}` → `DISPLAY\SHP155B\5&2f3a&0&UID261`。
fn device_path_to_instance(device_path: &str) -> String {
    let s = device_path.trim_start_matches("\\\\?\\");
    let s = match s.find("#{") {
        Some(i) => &s[..i],
        None => s,
    };
    s.replace('#', "\\")
}

/// 宽字符定长数组 → String（去尾部 NUL）。
pub(crate) fn wide_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// 哈希前 n 字节的 hex。
pub(crate) fn hex_prefix(hash: &[u8], n: usize) -> String {
    hash.iter().take(n).map(|b| format!("{:02x}", b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_path_conversion() {
        let p = r"\\?\DISPLAY#SHP155B#5&2f3a&0&UID261#{e6f07b5f-ee97-4a90-b076-33f57bf4eaa7}";
        assert_eq!(
            device_path_to_instance(p),
            r"DISPLAY\SHP155B\5&2f3a&0&UID261"
        );
    }

    #[test]
    fn wide_roundtrip() {
        let mut buf = [0u16; 8];
        for (i, c) in "ABC".encode_utf16().enumerate() {
            buf[i] = c;
        }
        assert_eq!(wide_to_string(&buf), "ABC");
    }

    #[test]
    fn hex_prefix_len() {
        let h = sha2::Sha256::digest(b"x");
        assert_eq!(hex_prefix(&h, 16).len(), 32);
    }
}
