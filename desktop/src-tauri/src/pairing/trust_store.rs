//! 信任存储（阶段 3）：持久化已配对设备的 Ed25519 公钥与元数据。
//!
//! 第一版用 JSON 文件存储（与 `device_identity.rs` 一致）。
//! 公钥本身非机密，不需要加密；后续可升级 OS keyring（见 05-pairing-security §5）。

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// 已信任设备。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrustedDevice {
    pub device_id: String,
    pub identity_pub_b64: String,
    #[serde(default)]
    pub name: Option<String>,
    /// 上次配对/连接的 unix 时间戳（秒）。
    #[serde(default)]
    pub last_seen: u64,
}

/// 信任存储（文件持久化）。
pub struct TrustStore {
    devices: Vec<TrustedDevice>,
    path: PathBuf,
}

impl TrustStore {
    /// 从 `path` 加载；文件不存在时创建空存储。
    pub fn load_or_create(path: PathBuf) -> std::io::Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let devices = if path.exists() {
            let data = fs::read_to_string(&path)?;
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            Vec::new()
        };
        Ok(Self { devices, path })
    }

    /// 内存存储（不持久化）；用于加载失败时的兜底。
    pub fn in_memory() -> Self {
        Self {
            devices: Vec::new(),
            path: PathBuf::new(),
        }
    }

    /// 列举所有已信任设备。
    pub fn list(&self) -> &[TrustedDevice] {
        &self.devices
    }

    /// 是否信任指定 device_id。
    pub fn is_trusted(&self, device_id: &str) -> bool {
        self.devices.iter().any(|d| d.device_id == device_id)
    }

    /// 查找已信任设备。
    pub fn get(&self, device_id: &str) -> Option<&TrustedDevice> {
        self.devices.iter().find(|d| d.device_id == device_id)
    }

    /// 添加或更新信任（按 device_id 去重，更新公钥/名称/时间）。
    pub fn add(&mut self, device: TrustedDevice) -> std::io::Result<()> {
        if let Some(existing) = self
            .devices
            .iter_mut()
            .find(|d| d.device_id == device.device_id)
        {
            *existing = device;
        } else {
            self.devices.push(device);
        }
        self.save()
    }

    /// 移除信任。
    pub fn remove(&mut self, device_id: &str) -> std::io::Result<bool> {
        let before = self.devices.len();
        self.devices.retain(|d| d.device_id != device_id);
        let removed = self.devices.len() < before;
        if removed {
            self.save()?;
        }
        Ok(removed)
    }

    /// 清空。
    pub fn clear(&mut self) -> std::io::Result<()> {
        self.devices.clear();
        self.save()
    }

    fn save(&self) -> std::io::Result<()> {
        // 空路径 = 内存模式，不持久化。
        if self.path.as_os_str().is_empty() {
            return Ok(());
        }
        let json = serde_json::to_string_pretty(&self.devices).map_err(std::io::Error::other)?;
        fs::write(&self.path, json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_path() -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "soundlink_trust_test_{}.json",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        p
    }

    #[test]
    fn add_and_persist() {
        let path = tmp_path();
        {
            let mut store = TrustStore::load_or_create(path.clone()).unwrap();
            assert!(!store.is_trusted("ios-ab12"));
            store
                .add(TrustedDevice {
                    device_id: "ios-ab12".into(),
                    identity_pub_b64: "base64pub".into(),
                    name: Some("My iPhone".into()),
                    last_seen: 1000,
                })
                .unwrap();
            assert!(store.is_trusted("ios-ab12"));
        }
        // 重新加载，确认持久化。
        let store = TrustStore::load_or_create(path.clone()).unwrap();
        assert!(store.is_trusted("ios-ab12"));
        assert_eq!(
            store.get("ios-ab12").unwrap().name.as_deref(),
            Some("My iPhone")
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn remove_works() {
        let path = tmp_path();
        let mut store = TrustStore::load_or_create(path.clone()).unwrap();
        store
            .add(TrustedDevice {
                device_id: "android-xx".into(),
                identity_pub_b64: "pub".into(),
                name: None,
                last_seen: 0,
            })
            .unwrap();
        assert!(store.remove("android-xx").unwrap());
        assert!(!store.is_trusted("android-xx"));
        let _ = fs::remove_file(path);
    }
}
