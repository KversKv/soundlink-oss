//! 设备指纹：单向哈希、无隐私信息、离线计算。
//!
//! 算法 v1：`base32(sha256("soundlink-fp-v1" || machine_id || device_id))[..10]`
//!
//! - `machine_id`：Windows 读 `HKLM\SOFTWARE\Microsoft\Cryptography\MachineGuid`，
//!   macOS 读 `IOPlatformUUID`，Linux 读 `/etc/machine-id`；**取不到即回退纯
//!   `device_id`，不报错**（E1：任何环境下指纹都必须算得出来）。
//! - `device_id`：`device_id.txt` 明文公开标识。
//!
//! C3：指纹算法带版本前缀且并行保留。算法变更时新版本同时计算 v1 与 v2，
//! 比对时任一命中即通过（`fingerprint_candidates`）。UI 旁须写明指纹为单向哈希。

use sha2::{Digest, Sha256};

use super::token::base32_encode;

/// 本机指纹候选集（首发仅 v1；比对使用 contains 语义，任一命中即通过）。
pub fn fingerprint_candidates(device_id: &str) -> Vec<String> {
    let mid = machine_id();
    vec![fingerprint_v1(mid.as_deref(), device_id)]
}

/// 指纹算法 v1（纯函数，便于单测与跨语言对齐）。
pub fn fingerprint_v1(machine_id: Option<&str>, device_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"soundlink-fp-v1");
    if let Some(mid) = machine_id {
        hasher.update(mid.trim().as_bytes());
    }
    hasher.update(device_id.trim().as_bytes());
    let digest = hasher.finalize();
    base32_encode(&digest)[..10].to_string()
}

/// 读取操作系统机器标识；失败返回 `None`（调用方回退纯 device_id）。
pub fn machine_id() -> Option<String> {
    platform_machine_id().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// Windows：注册表 MachineGuid。
/// 注册表读取依赖 `windows` crate（由 wasapi feature 引入）；未启用时回退 device_id。
#[cfg(all(windows, feature = "wasapi"))]
fn platform_machine_id() -> Option<String> {
    use windows::core::HSTRING;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_LOCAL_MACHINE, KEY_READ, REG_SZ,
        REG_VALUE_TYPE,
    };
    unsafe {
        let mut hkey = HKEY::default();
        let subkey = HSTRING::from("SOFTWARE\\Microsoft\\Cryptography");
        if RegOpenKeyExW(HKEY_LOCAL_MACHINE, &subkey, 0, KEY_READ, &mut hkey).is_err() {
            return None;
        }
        let name = HSTRING::from("MachineGuid");
        // 先查长度再读内容（REG_SZ，UTF-16）。
        let mut value_type = REG_VALUE_TYPE::default();
        let mut len: u32 = 0;
        let size_ok = RegQueryValueExW(
            hkey,
            &name,
            None,
            Some(&mut value_type),
            None,
            Some(&mut len),
        );
        if size_ok.is_err() || value_type != REG_SZ || len == 0 {
            let _ = RegCloseKey(hkey);
            return None;
        }
        let mut buf = vec![0u16; len as usize / 2 + 1];
        let read_ok = RegQueryValueExW(
            hkey,
            &name,
            None,
            None,
            Some(buf.as_mut_ptr() as *mut u8),
            Some(&mut len),
        );
        let _ = RegCloseKey(hkey);
        if read_ok.is_err() {
            return None;
        }
        let text = String::from_utf16_lossy(&buf)
            .trim_end_matches('\0')
            .to_string();
        Some(text)
    }
}

/// Linux：systemd / dbus 机器标识。
#[cfg(target_os = "linux")]
fn platform_machine_id() -> Option<String> {
    for path in ["/etc/machine-id", "/var/lib/dbus/machine-id"] {
        if let Ok(s) = std::fs::read_to_string(path) {
            let s = s.trim();
            if !s.is_empty() {
                return Some(s.to_string());
            }
        }
    }
    None
}

/// macOS：IOPlatformUUID（通过 ioreg 命令行读取，避免引入 IOKit 绑定）。
#[cfg(target_os = "macos")]
fn platform_machine_id() -> Option<String> {
    let out = std::process::Command::new("ioreg")
        .args(["-rd1", "-c", "IOPlatformExpertDevice"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        if let Some(pos) = line.find("\"IOPlatformUUID\"") {
            let rest = &line[pos..];
            if let Some(start) = rest.rfind('"') {
                // 形如 "IOPlatformUUID" = "XXXX-XXXX"
                let quote_start = rest[..start].rfind('"')?;
                return Some(rest[quote_start + 1..start].to_string());
            }
        }
    }
    None
}

/// 其余平台（含 Windows 未启用 wasapi feature 的核心构建）：回退纯 device_id。
#[cfg(not(any(
    all(windows, feature = "wasapi"),
    target_os = "linux",
    target_os = "macos"
)))]
fn platform_machine_id() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_inputs_same_fingerprint() {
        let a = fingerprint_v1(Some("MACHINE-GUID-1"), "pc-ab12");
        let b = fingerprint_v1(Some("MACHINE-GUID-1"), "pc-ab12");
        assert_eq!(a, b);
    }

    #[test]
    fn fingerprint_length_and_charset() {
        let fp = fingerprint_v1(Some("MACHINE-GUID-1"), "pc-ab12");
        assert_eq!(fp.len(), 10);
        assert!(fp
            .chars()
            .all(|c| c.is_ascii_uppercase() || ('2'..='7').contains(&c)));
    }

    #[test]
    fn different_device_different_fingerprint() {
        let a = fingerprint_v1(Some("MACHINE-GUID-1"), "pc-ab12");
        let b = fingerprint_v1(Some("MACHINE-GUID-1"), "pc-cd34");
        assert_ne!(a, b);
    }

    #[test]
    fn different_machine_different_fingerprint() {
        let a = fingerprint_v1(Some("MACHINE-GUID-1"), "pc-ab12");
        let b = fingerprint_v1(Some("MACHINE-GUID-2"), "pc-ab12");
        assert_ne!(a, b);
    }

    #[test]
    fn machine_id_missing_falls_back_silently() {
        // R3：machine_id 缺失回退纯 device_id，不报错且结果稳定。
        let a = fingerprint_v1(None, "pc-ab12");
        let b = fingerprint_v1(None, "pc-ab12");
        assert_eq!(a, b);
        assert_eq!(a.len(), 10);
        // 与有 machine_id 的结果不同（盐确实参与计算）。
        assert_ne!(a, fingerprint_v1(Some("MACHINE-GUID-1"), "pc-ab12"));
    }

    #[test]
    fn candidates_contain_v1_and_use_contains_semantics() {
        // R9：候选集含 v1，且比对为「任一命中即通过」。
        let cands = fingerprint_candidates("pc-test01");
        assert_eq!(cands.len(), 1);
        let v1 = fingerprint_v1(machine_id().as_deref(), "pc-test01");
        assert!(cands.contains(&v1));
    }
}
