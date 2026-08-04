//! 本机设备身份：首次运行生成 Ed25519 密钥对与稳定 device_id。
//! 用于发现 TXT 与配对信任。私钥通过 OS keyring（Windows Credential Manager /
//! macOS Keychain / Linux Secret Service）安全存储。
//!
//! P0 安全红线修复（NF-01 A3）：原 `identity.bin` 明文落盘迁移到 OS keyring。
//! 保留 `device_id.txt` 明文（device_id 是公开标识，非密钥）。
//! 旧 `identity.bin` 在 keyring 迁移成功后自动删除。

use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use std::fs;
use std::path::PathBuf;

/// OS keyring 服务名（统一标识 SoundLink）。
const KEYRING_SERVICE: &str = "soundlink";
/// Ed25519 私钥在 keyring 中的账号名。
const KEYRING_ACCOUNT_IDENTITY: &str = "device_identity_ed25519";

/// 设备身份。
#[derive(Debug)]
pub struct DeviceIdentity {
    pub device_id: String,
    pub signing_key: SigningKey,
}

impl DeviceIdentity {
    /// 公钥（Ed25519 verifying key）。
    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    /// 公钥 base64。
    pub fn identity_pub_b64(&self) -> String {
        use base64::{engine::general_purpose::STANDARD, Engine};
        STANDARD.encode(self.verifying_key().to_bytes())
    }

    /// 加载或生成并持久化。
    ///
    /// 优先从 OS keyring 读取私钥；若 keyring 不可用或不存在，回退检查旧 `identity.bin`
    /// 并迁移到 keyring；都没有则生成新身份并写入 keyring。
    pub fn load_or_create(dir: &PathBuf) -> std::io::Result<Self> {
        fs::create_dir_all(dir)?;
        let key_path = dir.join("identity.bin");
        let id_path = dir.join("device_id.txt");

        // 1. 优先从 keyring 读取。
        if let Some((key_bytes, id)) = load_from_keyring(&id_path) {
            if key_bytes.len() == 32 && !id.trim().is_empty() {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&key_bytes);
                let signing_key = SigningKey::from_bytes(&arr);
                tracing::info!("设备身份从 OS keyring 加载成功");
                return Ok(Self {
                    device_id: id.trim().to_string(),
                    signing_key,
                });
            }
        }

        // 2. 回退：从旧 identity.bin 迁移。
        if key_path.exists() && id_path.exists() {
            let key_bytes = fs::read(&key_path)?;
            let id = fs::read_to_string(&id_path)?;
            if key_bytes.len() == 32 && !id.trim().is_empty() {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&key_bytes);
                let signing_key = SigningKey::from_bytes(&arr);
                // 尝试迁移到 keyring；失败则保留文件作为兜底。
                if save_to_keyring(&key_bytes).is_ok() {
                    tracing::info!("设备身份已从 identity.bin 迁移到 OS keyring");
                    // 迁移成功后删除明文私钥文件。
                    let _ = fs::remove_file(&key_path);
                } else {
                    tracing::warn!("keyring 写入失败，保留 identity.bin 作为兜底");
                }
                return Ok(Self {
                    device_id: id.trim().to_string(),
                    signing_key,
                });
            }
        }

        // 3. 生成新身份。
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let device_id = format!("pc-{}", hex_short(&signing_key.verifying_key().to_bytes()));
        let key_bytes = signing_key.to_bytes();

        // 优先写 keyring；失败则回退到文件存储（保证可用性）。
        if save_to_keyring(&key_bytes).is_err() {
            tracing::warn!("keyring 写入失败，回退到 identity.bin 文件存储");
            fs::write(&key_path, key_bytes)?;
        }
        fs::write(&id_path, &device_id)?;
        Ok(Self {
            device_id,
            signing_key,
        })
    }

    /// 持久化临时身份（D5）：加载失败后用临时身份调用，尝试写盘避免重启后身份变化。
    /// 不覆盖已存在的 device_id.txt；私钥写 keyring，失败则写 identity.bin 兜底。
    pub fn try_persist_temp(&self, dir: &PathBuf) -> std::io::Result<()> {
        fs::create_dir_all(dir)?;
        let id_path = dir.join("device_id.txt");
        if !id_path.exists() {
            fs::write(&id_path, &self.device_id)?;
        }
        let key_bytes = self.signing_key.to_bytes();
        if save_to_keyring(&key_bytes).is_err() {
            tracing::warn!("临时身份 keyring 写入失败，回退到 identity.bin 文件存储");
            let key_path = dir.join("identity.bin");
            if !key_path.exists() {
                fs::write(&key_path, key_bytes)?;
            }
        }
        Ok(())
    }
}

/// 从 keyring 读取私钥；同时从 `device_id.txt` 读取 device_id。
/// keyring 不可用时返回 None。
fn load_from_keyring(id_path: &PathBuf) -> Option<(Vec<u8>, String)> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT_IDENTITY).ok()?;
    let secret = entry.get_secret().ok()?;
    let id = fs::read_to_string(id_path).ok()?;
    Some((secret.to_vec(), id))
}

/// 写入私钥到 keyring。返回 io::Result 以便调用方统一错误处理。
fn save_to_keyring(key_bytes: &[u8]) -> std::io::Result<()> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT_IDENTITY)
        .map_err(|e| std::io::Error::other(format!("keyring entry 创建失败：{}", e)))?;
    entry
        .set_secret(key_bytes)
        .map_err(|e| std::io::Error::other(format!("keyring 写入失败：{}", e)))
}

fn hex_short(b: &[u8]) -> String {
    b.iter().take(3).map(|x| format!("{:02x}", x)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_and_pub() {
        let mut csprng = OsRng;
        let sk = SigningKey::generate(&mut csprng);
        let _vk = sk.verifying_key();
        assert_eq!(sk.to_bytes().len(), 32);
    }
}
