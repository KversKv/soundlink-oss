//! EDID 注册表读写（HKLM）。
//!
//! - 读取：普通权限即可（`HKLM\...\Enum` 用户可读）；
//! - 写入/删除 EDID_OVERRIDE：需提权，**仅 qr_helper 调用**（主进程不直接调）。
//!
//! 注册表变体（display.md §5.1，探测得出、不硬编码生效项）：
//! 1. 显示器实例 `Device Parameters\EDID_OVERRIDE`（CRU 同款，最常用）；
//! 2. 监视器类键 `Control\Class\{4d36e96e-...}\NNNN\EDID_OVERRIDE`（旧式）；
//! 3. `GraphicsDrivers\Configuration\<匹配键>\00`（驱动相关兜底，探测阶梯验证）。

use crate::features::quick_resolution::model::QrError;
use qr_ipc::RegVariant;
use windows::core::PCWSTR;
use windows::Win32::Foundation::*;
use windows::Win32::System::Registry::*;

const ENUM_ROOT: &str = r"SYSTEM\CurrentControlSet\Enum";
const CLASS_ROOT: &str = r"SYSTEM\CurrentControlSet\Control\Class";
const GDR_CONFIG_ROOT: &str = r"SYSTEM\CurrentControlSet\Control\GraphicsDrivers\Configuration";

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

fn win32_err(api: &str, r: WIN32_ERROR) -> QrError {
    QrError::Win32 { api: api.into(), code: r.0 as i32 }
}

/// 读取注册表二进制值（HKLM，只读）。
fn read_binary(subkey: &str, value: &str) -> Result<Vec<u8>, QrError> {
    unsafe {
        let sub = wide(subkey);
        let mut hkey = HKEY::default();
        let r = RegOpenKeyExW(HKEY_LOCAL_MACHINE, PCWSTR(sub.as_ptr()), 0, KEY_READ, &mut hkey);
        if r != ERROR_SUCCESS {
            return Err(win32_err("RegOpenKeyExW(read)", r));
        }
        let val = wide(value);
        let mut len = 0u32;
        let r = RegQueryValueExW(hkey, PCWSTR(val.as_ptr()), None, None, None, Some(&mut len));
        if r != ERROR_SUCCESS {
            let _ = RegCloseKey(hkey);
            return Err(win32_err("RegQueryValueEx(size)", r));
        }
        let mut buf = vec![0u8; len as usize];
        let r = RegQueryValueExW(
            hkey,
            PCWSTR(val.as_ptr()),
            None,
            None,
            Some(buf.as_mut_ptr()),
            Some(&mut len),
        );
        let _ = RegCloseKey(hkey);
        if r != ERROR_SUCCESS {
            return Err(win32_err("RegQueryValueEx(data)", r));
        }
        buf.truncate(len as usize);
        Ok(buf)
    }
}

/// 读取字符串值（REG_SZ）。
fn read_string(subkey: &str, value: &str) -> Result<String, QrError> {
    let bytes = read_binary(subkey, value)?;
    let u16s: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let end = u16s.iter().position(|&c| c == 0).unwrap_or(u16s.len());
    Ok(String::from_utf16_lossy(&u16s[..end]))
}

/// 写入二进制值（**需提权**，仅 helper 调用）。
pub fn write_binary(subkey: &str, value: &str, data: &[u8]) -> Result<(), QrError> {
    unsafe {
        let sub = wide(subkey);
        let mut hkey = HKEY::default();
        let mut disp = REG_CREATE_KEY_DISPOSITION(0);
        let r = RegCreateKeyExW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(sub.as_ptr()),
            0,
            None,
            REG_OPEN_CREATE_OPTIONS(0),
            KEY_WRITE,
            None,
            &mut hkey,
            Some(&mut disp),
        );
        if r != ERROR_SUCCESS {
            return Err(win32_err("RegCreateKeyExW", r));
        }
        let val = wide(value);
        let r = RegSetValueExW(hkey, PCWSTR(val.as_ptr()), 0, REG_BINARY, Some(data));
        let _ = RegCloseKey(hkey);
        if r != ERROR_SUCCESS {
            return Err(win32_err("RegSetValueExW", r));
        }
        Ok(())
    }
}

/// 删除值（**需提权**，仅 helper 调用）。值不存在视为成功（幂等）。
pub fn delete_value(subkey: &str, value: &str) -> Result<(), QrError> {
    unsafe {
        let sub = wide(subkey);
        let mut hkey = HKEY::default();
        let r = RegOpenKeyExW(HKEY_LOCAL_MACHINE, PCWSTR(sub.as_ptr()), 0, KEY_WRITE, &mut hkey);
        if r != ERROR_SUCCESS {
            return if r == ERROR_FILE_NOT_FOUND {
                Ok(())
            } else {
                Err(win32_err("RegOpenKeyExW(write)", r))
            };
        }
        let val = wide(value);
        let r = RegDeleteValueW(hkey, PCWSTR(val.as_ptr()));
        let _ = RegCloseKey(hkey);
        if r == ERROR_SUCCESS || r == ERROR_FILE_NOT_FOUND {
            Ok(())
        } else {
            Err(win32_err("RegDeleteValueW", r))
        }
    }
}

/// 枚举子键名（GraphicsDrivers\Configuration 匹配用）。
fn enum_subkeys(subkey: &str) -> Vec<String> {
    unsafe {
        let sub = wide(subkey);
        let mut hkey = HKEY::default();
        let r = RegOpenKeyExW(HKEY_LOCAL_MACHINE, PCWSTR(sub.as_ptr()), 0, KEY_READ, &mut hkey);
        if r != ERROR_SUCCESS {
            return Vec::new();
        }
        let mut out = Vec::new();
        let mut i = 0u32;
        loop {
            let mut buf = [0u16; 256];
            let mut len = buf.len() as u32;
            let r = RegEnumKeyExW(
                hkey,
                i,
                windows::core::PWSTR(buf.as_mut_ptr()),
                &mut len,
                None,
                windows::core::PWSTR::null(),
                None,
                None,
            );
            if r != ERROR_SUCCESS {
                break;
            }
            out.push(String::from_utf16_lossy(&buf[..len as usize]));
            i += 1;
        }
        let _ = RegCloseKey(hkey);
        out
    }
}

/// 显示器实例的 Device Parameters 子键路径（HKLM 相对）。
fn device_params_subkey(instance_path: &str) -> String {
    format!(r"{}\{}\Device Parameters", ENUM_ROOT, instance_path)
}

/// 变体对应的注册表子键路径（HKLM 相对）。
pub fn resolve_variant_subkey(instance_path: &str, variant: RegVariant) -> Result<String, QrError> {
    match variant {
        RegVariant::MonitorInstanceOverride => Ok(device_params_subkey(instance_path)),
        RegVariant::ClassMonitorOverride => {
            // Enum 设备键的 "Driver" 值 = "{4d36e96e-...}\NNNN"。
            let enum_key = format!(r"{}\{}", ENUM_ROOT, instance_path);
            let driver = read_string(&enum_key, "Driver")
                .map_err(|_| QrError::Edid("无法定位显示器类键（Driver 值缺失）".into()))?;
            Ok(format!(r"{}\{}", CLASS_ROOT, driver))
        }
        RegVariant::GraphicsDriversConfiguration => {
            // 以原生 EDID 的 厂商+产品码 前缀匹配 Configuration 子键，附加 \00。
            let edid = read_native_edid(instance_path)?;
            let info = qr_edid::parse::parse(&edid).map_err(QrError::from)?;
            let prefix = format!("{}{:X}", info.manufacturer, info.product_code);
            for sub in enum_subkeys(GDR_CONFIG_ROOT) {
                if sub.to_uppercase().starts_with(&prefix.to_uppercase()) {
                    return Ok(format!(r"{}\{}\00", GDR_CONFIG_ROOT, sub));
                }
            }
            Err(QrError::Edid(format!(
                "GraphicsDrivers\\Configuration 中无匹配键（前缀 {}）",
                prefix
            )))
        }
    }
}

/// `.reg` 还原文件用的完整路径（含 HKEY_LOCAL_MACHINE 前缀）。
pub fn variant_full_path_for_reg(instance_path: &str, variant: RegVariant) -> String {
    match resolve_variant_subkey(instance_path, variant) {
        Ok(sub) => format!(r"HKEY_LOCAL_MACHINE\{}", sub),
        Err(_) => format!(r"HKEY_LOCAL_MACHINE\{}", device_params_subkey(instance_path)),
    }
}

/// override 值名（实例/类键变体）。GraphicsDrivers 变体写 "EDID"（驱动相关兜底）。
fn override_value_name(variant: RegVariant) -> &'static str {
    match variant {
        RegVariant::GraphicsDriversConfiguration => "EDID",
        _ => "EDID_OVERRIDE",
    }
}

/// 读取原生 EDID（Enum\<instance>\Device Parameters\EDID）。
pub fn read_native_edid(instance_path: &str) -> Result<Vec<u8>, QrError> {
    read_binary(&device_params_subkey(instance_path), "EDID")
}

/// 读取生效 EDID（override 优先）。
pub fn read_effective_edid(instance_path: &str) -> Result<Vec<u8>, QrError> {
    // 实例变体优先（最常用）；类键其次。
    for variant in [RegVariant::MonitorInstanceOverride, RegVariant::ClassMonitorOverride] {
        if let Ok(sub) = resolve_variant_subkey(instance_path, variant) {
            if let Ok(data) = read_binary(&sub, override_value_name(variant)) {
                if !data.is_empty() {
                    return Ok(data);
                }
            }
        }
    }
    read_native_edid(instance_path)
}

/// 写 EDID Override（helper）。
pub fn write_override(instance_path: &str, variant: RegVariant, edid: &[u8]) -> Result<String, QrError> {
    let sub = resolve_variant_subkey(instance_path, variant)?;
    write_binary(&sub, override_value_name(variant), edid)?;
    Ok(sub)
}

/// 删除 EDID Override（helper）。
pub fn remove_override(instance_path: &str, variant: RegVariant) -> Result<(), QrError> {
    let sub = resolve_variant_subkey(instance_path, variant)?;
    delete_value(&sub, override_value_name(variant))
}

/// 读取当前 override（helper 备份前读取原值用）。
pub fn read_override(instance_path: &str, variant: RegVariant) -> Result<Vec<u8>, QrError> {
    let sub = resolve_variant_subkey(instance_path, variant)?;
    read_binary(&sub, override_value_name(variant))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subkey_layout() {
        let s = device_params_subkey(r"DISPLAY\LGS1234\5&2F3A");
        assert_eq!(
            s,
            r"SYSTEM\CurrentControlSet\Enum\DISPLAY\LGS1234\5&2F3A\Device Parameters"
        );
    }

    #[test]
    fn reg_full_path_fallback() {
        // 无 Driver 值环境下应回退到实例变体路径（本测试环境通常无此显示器键）。
        let p = variant_full_path_for_reg(r"DISPLAY\X\1", RegVariant::MonitorInstanceOverride);
        assert!(p.starts_with(r"HKEY_LOCAL_MACHINE\SYSTEM\CurrentControlSet\Enum\DISPLAY\X\1"));
    }
}
