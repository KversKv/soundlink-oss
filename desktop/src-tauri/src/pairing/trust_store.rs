//! 信任存储（阶段 3 完整实现）。阶段 1：占位。
//! 后续使用 OS keyring 或加密本地存储保存已配对设备 Ed25519 公钥与元数据。

#[derive(Debug, Clone)]
pub struct TrustedDevice {
    pub device_id: String,
    pub identity_pub_b64: String,
    pub name: Option<String>,
}

/// 内存占位信任表。
pub struct TrustStore {
    _inner: Vec<TrustedDevice>,
}

impl Default for TrustStore {
    fn default() -> Self {
        Self::new()
    }
}

impl TrustStore {
    pub fn new() -> Self {
        Self { _inner: Vec::new() }
    }
}
