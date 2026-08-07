//! Pro 授权：离线 Ed25519 签名许可证（MON-01 阶段 R）。
//!
//! 红线：
//! - E1：任何校验失败一律**降级为免费版**，绝不阻止启动或中断音频；
//! - E2：全程离线，不联网、不上报任何信息；
//! - E8：已签发的 license 在所有后续版本中永久有效（兼容约束见 token.rs 头部 C1–C5）。
//!
//! 存储：license 文本存 OS keyring（`service="soundlink"`, `account="pro_license"`），
//! 兜底 `<config_dir>/license.key`。
//! 注意：与 `fixed_pairing_code` 不同，license **允许明文文件兜底**——它不是用户的
//! 安全凭据，泄露只影响作者收入。此处明文兜底是有意为之，并非安全缺陷。

pub mod fingerprint;
pub mod revocation;
pub mod token;

use std::path::Path;

/// OS keyring 服务名（与设备身份/固定配对码共用；**永不可改**，G8）。
const KEYRING_SERVICE: &str = "soundlink";
/// license 在 keyring 中的账号名（**永不可改**，G8）。
const KEYRING_ACCOUNT_LICENSE: &str = "pro_license";
/// 兜底文件名（位于配置目录，与 app_config.json 同目录）。
const LICENSE_FILE: &str = "license.key";

/// 授权校验结论。`Free` 是正常状态（未购买/未激活），不是错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LicenseState {
    /// 无 license（初始状态）。
    Free,
    /// 校验通过。
    Active { sub: String, iat: u64, seats: u8 },
    /// 格式/签名/内容无效。
    Invalid(String),
    /// 已过 exp（买断 key 永不会出现）。
    Expired,
    /// nonce 命中吊销名单。
    Revoked,
    /// 指纹绑定但与本机不符（换机未重签）。
    DeviceMismatch,
}

impl LicenseState {
    pub fn is_active(&self) -> bool {
        matches!(self, LicenseState::Active { .. })
    }

    /// 前端展示用状态码。
    pub fn state_str(&self) -> &'static str {
        match self {
            LicenseState::Free => "free",
            LicenseState::Active { .. } => "active",
            LicenseState::Invalid(_) => "invalid",
            LicenseState::Expired => "expired",
            LicenseState::Revoked => "revoked",
            LicenseState::DeviceMismatch => "device_mismatch",
        }
    }

    /// `Invalid` 的详细原因（其余状态为 None）。
    pub fn detail(&self) -> Option<&str> {
        match self {
            LicenseState::Invalid(reason) => Some(reason),
            _ => None,
        }
    }

    /// 激活信息（供 UI 回显，调用方负责掩码）。
    pub fn active_sub(&self) -> Option<&str> {
        match self {
            LicenseState::Active { sub, .. } => Some(sub),
            _ => None,
        }
    }
}

/// 当前 Unix 秒。
fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 校验 license 文本（使用内置 vendor 公钥与吊销名单，当前系统时间）。
pub fn validate(raw: &str, device_id: &str) -> LicenseState {
    let fps = fingerprint::fingerprint_candidates(device_id);
    match token::validate_token(
        raw,
        now_secs(),
        &fps,
        &token::vendor_keys(),
        revocation::REVOKED_NONCES,
        token::LICENSE_FORMAT_MAX,
    ) {
        Ok(p) => LicenseState::Active {
            sub: p.sub,
            iat: p.iat,
            seats: p.seats,
        },
        Err(token::Reject::Invalid(reason)) => LicenseState::Invalid(reason),
        Err(token::Reject::Expired) => LicenseState::Expired,
        Err(token::Reject::Revoked) => LicenseState::Revoked,
        Err(token::Reject::DeviceMismatch) => LicenseState::DeviceMismatch,
    }
}

/// 读取已存 license 文本：keyring 优先，文件兜底。
///
/// `Free` 是正常状态：读取失败仅 `tracing::info!`，不 warn 不 error（R4）。
pub fn load_license_text(dir: &Path) -> Option<String> {
    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT_LICENSE) {
        match entry.get_secret() {
            Ok(secret) => {
                if let Ok(text) = String::from_utf8(secret.to_vec()) {
                    if !text.trim().is_empty() {
                        return Some(text.trim().to_string());
                    }
                }
                tracing::info!("keyring 中 license 为空或非 UTF-8，尝试文件兜底");
            }
            Err(keyring::Error::NoEntry) => {}
            Err(e) => tracing::info!("keyring 读取 license 失败：{}，尝试文件兜底", e),
        }
    }
    let path = dir.join(LICENSE_FILE);
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            let text = text.trim().to_string();
            if text.is_empty() {
                None
            } else {
                Some(text)
            }
        }
        Err(_) => None,
    }
}

/// 写入 license 文本：优先 keyring；keyring 不可用时写文件兜底。
pub fn save_license_text(dir: &Path, key: &str) -> Result<(), String> {
    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT_LICENSE) {
        if entry.set_secret(key.as_bytes()).is_ok() {
            // keyring 写成功：清理旧文件兜底，避免两份文本漂移。
            let _ = std::fs::remove_file(dir.join(LICENSE_FILE));
            return Ok(());
        }
    }
    // 文件兜底（见模块头注释：license 允许明文兜底）。
    std::fs::create_dir_all(dir).map_err(|e| format!("创建配置目录失败：{}", e))?;
    std::fs::write(dir.join(LICENSE_FILE), key).map_err(|e| format!("写入 license 文件失败：{}", e))
}

/// 清除已存 license（keyring 与文件都清）。
pub fn clear_license(dir: &Path) {
    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT_LICENSE) {
        let _ = entry.delete_credential();
    }
    let _ = std::fs::remove_file(dir.join(LICENSE_FILE));
}

/// 启动时加载并验签一次（R4）。无 license → `Free`（正常状态）。
pub fn load_and_validate(dir: &Path, device_id: &str) -> LicenseState {
    match load_license_text(dir) {
        None => LicenseState::Free,
        Some(text) => {
            let state = validate(&text, device_id);
            match &state {
                LicenseState::Active { .. } => {
                    tracing::info!("Pro 授权校验通过（离线验签）")
                }
                other => tracing::info!(
                    "license 存在但未激活（state={}），按免费版运行",
                    other.state_str()
                ),
            }
            state
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir() -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "soundlink_license_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn state_str_mapping() {
        assert_eq!(LicenseState::Free.state_str(), "free");
        assert_eq!(
            LicenseState::Active {
                sub: "s".into(),
                iat: 0,
                seats: 3
            }
            .state_str(),
            "active"
        );
        assert_eq!(LicenseState::Invalid("x".into()).state_str(), "invalid");
        assert_eq!(LicenseState::Expired.state_str(), "expired");
        assert_eq!(LicenseState::Revoked.state_str(), "revoked");
        assert_eq!(LicenseState::DeviceMismatch.state_str(), "device_mismatch");
    }

    #[test]
    fn no_license_is_free() {
        let dir = tmp_dir();
        // 注意：若运行环境 keyring 恰好存有 pro_license 条目此测试会受影响；
        // CI/开发机上不存在该条目，且断言只针对文件兜底路径。
        assert_eq!(load_license_text(&dir), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn file_fallback_roundtrip() {
        // R10：文件兜底路径「旧版本写入 → 新版本读取」等价于文本原样可读。
        let dir = tmp_dir();
        // 绕开 keyring 直接写文件，模拟旧版本遗留的 license.key。
        std::fs::write(dir.join(LICENSE_FILE), "SLPRO-ABC-DEF\n").unwrap();
        assert_eq!(load_license_text(&dir), Some("SLPRO-ABC-DEF".into()));
        clear_license(&dir);
        assert_eq!(load_license_text(&dir), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_file_is_none() {
        let dir = tmp_dir();
        std::fs::write(dir.join(LICENSE_FILE), "   \n").unwrap();
        assert_eq!(load_license_text(&dir), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
