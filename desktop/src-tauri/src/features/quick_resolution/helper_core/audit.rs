//! helper 审计日志（display.md §4.2「幂等 + 审计」）。
//!
//! 独立文件 `helper.log.<date>`，落 `%APPDATA%/soundlink/logs/`（与主程序日志同目录）。
//! 每条写操作记录操作前后 EDID 哈希（SHA-256 前 8 字节 hex）。

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

/// 日志目录（与主程序 logging::log_dir 同一约定）。
fn log_dir() -> PathBuf {
    let mut p = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    p.push("soundlink");
    p.push("logs");
    p
}

fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let (y, m, d) = civil(days);
    format!("{:04}-{:02}-{:02}", y, m, d)
}

fn now_hms() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let rem = secs % 86_400;
    format!("{:02}:{:02}:{:02}", rem / 3600, (rem % 3600) / 60, rem % 60)
}

fn civil(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// EDID 哈希摘要（审计用，SHA-256 前 8 字节 hex）。
pub fn edid_digest(edid: &[u8]) -> String {
    use sha2::Digest;
    let h = sha2::Sha256::digest(edid);
    h.iter().take(8).map(|b| format!("{:02x}", b)).collect()
}

/// 追加一条审计记录。
pub fn log(op: &str, detail: &str) {
    let dir = log_dir();
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join(format!("helper.log.{}", today()));
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "[{}] {} | {}", now_hms(), op, detail);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_stable() {
        assert_eq!(edid_digest(&[0u8; 128]).len(), 16);
        assert_eq!(edid_digest(b"abc"), edid_digest(b"abc"));
        assert_ne!(edid_digest(b"abc"), edid_digest(b"abd"));
    }

    #[test]
    fn date_shape() {
        assert_eq!(today().len(), 10);
        assert_eq!(now_hms().len(), 8);
    }
}
