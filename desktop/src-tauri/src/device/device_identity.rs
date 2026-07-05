//! 本机设备身份：首次运行生成 Ed25519 密钥对与稳定 device_id。
//! 用于发现 TXT 与配对信任。私钥本地安全存储（第一版文件存储，后续升级 OS keyring）。

use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use std::fs;
use std::path::PathBuf;

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
    pub fn load_or_create(dir: &PathBuf) -> std::io::Result<Self> {
        fs::create_dir_all(dir)?;
        let key_path = dir.join("identity.bin");
        let id_path = dir.join("device_id.txt");
        if key_path.exists() && id_path.exists() {
            let key_bytes = fs::read(&key_path)?;
            let id = fs::read_to_string(&id_path)?;
            if key_bytes.len() == 32 && !id.trim().is_empty() {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&key_bytes);
                let signing_key = SigningKey::from_bytes(&arr);
                return Ok(Self {
                    device_id: id.trim().to_string(),
                    signing_key,
                });
            }
        }
        // 生成新身份。
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let device_id = format!("pc-{}", hex_short(&signing_key.verifying_key().to_bytes()));
        fs::write(&key_path, signing_key.to_bytes())?;
        fs::write(&id_path, &device_id)?;
        Ok(Self {
            device_id,
            signing_key,
        })
    }
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
