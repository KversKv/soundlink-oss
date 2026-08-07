//! 信任存储（阶段 3）：持久化已配对设备的 Ed25519 公钥与元数据。
//!
//! 第一版用 JSON 文件存储（与 `device_identity.rs` 一致）。
//! 公钥本身非机密，不需要加密；后续可升级 OS keyring（见 05-pairing-security §5）。

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// 已信任设备。
///
/// 同时承载两种视角：
/// - 接收端视角（信任发送端）：`host` 为 `None`
/// - 发送端视角（信任接收端）：`host` 为 `Some(ip)`，附带端口信息
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrustedDevice {
    pub device_id: String,
    pub identity_pub_b64: String,
    #[serde(default)]
    pub name: Option<String>,
    /// 上次配对/连接的 unix 时间戳（秒）。
    #[serde(default)]
    pub last_seen: u64,
    /// 发送端视角：Receiver 的 IP（host）。`None` 表示接收端视角的信任条目。
    #[serde(default)]
    pub host: Option<String>,
    /// 发送端视角：Receiver 控制端口。
    #[serde(default)]
    pub control_port: Option<u16>,
    /// 发送端视角：Receiver 音频端口。
    #[serde(default)]
    pub audio_port: Option<u16>,
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
    ///
    /// MON-01 S1：`max` 为**同方向**（接收端信任的发送端 `host=None` /
    /// 发送端信任的接收端 `host=Some` 各自独立计数）容量上限。
    /// 超限时**替换 `last_seen` 最旧的同方向条目**而非拒绝新配对
    /// （拒绝会让「换手机」场景卡死；替换最旧符合「记住最近用的那台」的直觉）。
    /// 返回被替换掉的条目（调用方据此通知 UI）；`max=0` 时不存储。
    pub fn add(&mut self, device: TrustedDevice, max: usize) -> std::io::Result<Option<TrustedDevice>> {
        if let Some(existing) = self
            .devices
            .iter_mut()
            .find(|d| d.device_id == device.device_id)
        {
            *existing = device;
            self.save()?;
            return Ok(None);
        }
        if max == 0 {
            return Ok(None);
        }
        let same_view = |d: &TrustedDevice| d.host.is_some() == device.host.is_some();
        let mut evicted = None;
        if self.devices.iter().filter(|d| same_view(d)).count() >= max {
            if let Some((idx, _)) = self
                .devices
                .iter()
                .enumerate()
                .filter(|(_, d)| same_view(d))
                .min_by_key(|(_, d)| d.last_seen)
            {
                evicted = Some(self.devices.remove(idx));
            }
        }
        self.devices.push(device);
        self.save()?;
        Ok(evicted)
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

    fn device(id: &str, last_seen: u64, host: Option<&str>) -> TrustedDevice {
        TrustedDevice {
            device_id: id.into(),
            identity_pub_b64: "pub".into(),
            name: Some(format!("设备-{}", id)),
            last_seen,
            host: host.map(String::from),
            control_port: host.map(|_| 47820),
            audio_port: host.map(|_| 47821),
        }
    }

    #[test]
    fn add_and_persist() {
        let path = tmp_path();
        {
            let mut store = TrustStore::load_or_create(path.clone()).unwrap();
            assert!(!store.is_trusted("ios-ab12"));
            store
                .add(
                    TrustedDevice {
                        device_id: "ios-ab12".into(),
                        identity_pub_b64: "base64pub".into(),
                        name: Some("My iPhone".into()),
                        last_seen: 1000,
                        host: None,
                        control_port: None,
                        audio_port: None,
                    },
                    8,
                )
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
            .add(
                TrustedDevice {
                    device_id: "android-xx".into(),
                    identity_pub_b64: "pub".into(),
                    name: None,
                    last_seen: 0,
                    host: None,
                    control_port: None,
                    audio_port: None,
                },
                8,
            )
            .unwrap();
        assert!(store.remove("android-xx").unwrap());
        assert!(!store.is_trusted("android-xx"));
        let _ = fs::remove_file(path);
    }

    // ── MON-01 S1：容量约束（免费 1 / Pro 8，同方向独立计数） ──

    #[test]
    fn cap_one_replaces_oldest_same_view() {
        // 免费上限 1：第 2 台同方向设备替换第 1 台（最旧），并返回被替换条目。
        let mut store = TrustStore::in_memory();
        assert!(store.add(device("phone-a", 1000, None), 1).unwrap().is_none());
        let evicted = store.add(device("phone-b", 2000, None), 1).unwrap();
        assert_eq!(evicted.map(|d| d.device_id), Some("phone-a".into()));
        assert!(!store.is_trusted("phone-a"));
        assert!(store.is_trusted("phone-b"));
        assert_eq!(store.list().len(), 1);
    }

    #[test]
    fn cap_eight_accumulates_then_replaces_oldest() {
        // Pro 上限 8：累积到 8 后才替换最旧。
        let mut store = TrustStore::in_memory();
        for i in 0..8 {
            let id = format!("recv-{}", i);
            assert!(store.add(device(&id, 1000 + i, Some("192.168.1.2")), 8).unwrap().is_none());
        }
        assert_eq!(store.list().len(), 8);
        let evicted = store
            .add(device("recv-new", 9000, Some("192.168.1.9")), 8)
            .unwrap();
        assert_eq!(evicted.map(|d| d.device_id), Some("recv-0".into()));
        assert_eq!(store.list().len(), 8);
        assert!(store.is_trusted("recv-new"));
        assert!(store.is_trusted("recv-1"));
    }

    #[test]
    fn caps_are_per_direction_independent() {
        // 两个方向独立计数：上限 1 时可同时记住 1 台发送端 + 1 台接收端。
        let mut store = TrustStore::in_memory();
        store.add(device("phone-a", 1000, None), 1).unwrap();
        store.add(device("recv-a", 1000, Some("192.168.1.2")), 1).unwrap();
        assert_eq!(store.list().len(), 2);
        // 发送端方向新增会顶掉旧发送端，但不影响接收端方向。
        let evicted = store.add(device("phone-b", 2000, None), 1).unwrap();
        assert_eq!(evicted.map(|d| d.device_id), Some("phone-a".into()));
        assert!(store.is_trusted("recv-a"));
        assert_eq!(store.list().len(), 2);
    }

    #[test]
    fn update_existing_does_not_evict() {
        // 更新既有条目（同 device_id）不触发替换，只刷新 last_seen。
        let mut store = TrustStore::in_memory();
        store.add(device("phone-a", 1000, None), 1).unwrap();
        assert!(store.add(device("phone-a", 5000, None), 1).unwrap().is_none());
        assert_eq!(store.list().len(), 1);
        assert_eq!(store.get("phone-a").unwrap().last_seen, 5000);
        // 刷新后它成为「最新」，新设备仍会顶掉它（上限 1）。
        let evicted = store.add(device("phone-b", 6000, None), 1).unwrap();
        assert_eq!(evicted.map(|d| d.device_id), Some("phone-a".into()));
    }
}
