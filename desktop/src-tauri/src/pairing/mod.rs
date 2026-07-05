// pairing/mod.rs — 占位
pub mod pairing_code;  // 生成/校验配对码 (6~8位, 120s, 3~5次)
pub mod key_exchange;  // X25519 会话密钥 (后续 SPAKE2/SRP)
pub mod trust_store;   // 设备身份公钥信任存储
