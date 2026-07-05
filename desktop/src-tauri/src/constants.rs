//! 全局常量（单源）。对齐 `shared/constants/README.md` 与
//! `docs/First/11-implementation-spec.md` §1。改这里同步改文档。

/// mDNS 服务类型。
pub const MDNS_SERVICE_TYPE: &str = "_soundlink._udp.local.";

/// 协议版本。
pub const PROTOCOL_VERSION: u8 = 1;

/// AudioPacket 魔数 "SL"（大端）。
pub const MAGIC: u16 = 0x534C;

/// AudioPacket 固定头部长度（字节）。
pub const HEADER_LEN: u8 = 32;

/// 默认控制通道端口（TCP/WS）。
pub const DEFAULT_CONTROL_PORT: u16 = 47810;

/// 默认音频通道端口（UDP）。
pub const DEFAULT_AUDIO_PORT: u16 = 47811;

/// 采样率。
pub const SAMPLE_RATE: u32 = 48_000;

/// 声道数。
pub const CHANNELS: u8 = 2;

/// Opus 帧长（毫秒）。
pub const FRAME_DURATION_MS: u8 = 10;

/// 每帧每声道样本数 = 48000 * 10 / 1000。
pub const SAMPLES_PER_FRAME_PER_CHANNEL: usize = 480;

/// Opus 起始码率。
pub const OPUS_BITRATE: u32 = 128_000;

/// 编码类型：Opus。
pub const CODEC_OPUS: u8 = 1;

/// flags bit0：流末包。
pub const FLAG_STREAM_END: u8 = 0x01;

/// 默认 Jitter 缓冲（毫秒）。
pub const DEFAULT_JITTER_MS: u32 = 80;

/// Jitter 三档（毫秒）。
pub const JITTER_LOW_MS: u32 = 40;
pub const JITTER_BALANCED_MS: u32 = 80;
pub const JITTER_STABLE_MS: u32 = 150;

/// 配对码位数。
pub const PAIRING_CODE_DIGITS: usize = 8;

/// 配对码有效期（秒）。
pub const PAIRING_CODE_TTL_SECS: u64 = 120;

/// 配对码最大尝试次数。
pub const PAIRING_CODE_MAX_ATTEMPTS: u32 = 5;

/// AEAD 密钥长度（字节）。
pub const AEAD_KEY_LEN: usize = 32;

/// AEAD nonce 长度（字节）。
pub const AEAD_NONCE_LEN: usize = 12;

/// AEAD 认证标签长度（字节）。
pub const AEAD_TAG_LEN: usize = 16;

/// 心跳间隔（秒）。
pub const HEARTBEAT_INTERVAL_SECS: u64 = 2;

/// 心跳超时（秒）。
pub const HEARTBEAT_TIMEOUT_SECS: u64 = 6;

/// HKDF 派生 salt。
pub const PAIRING_SALT: &[u8] = b"soundlink-pair-v1";

/// 会话密钥 HKDF info。
pub const SESSION_INFO: &[u8] = b"soundlink-session-v1";

/// 音频密钥 HKDF info。
pub const AUDIO_KEY_INFO: &[u8] = b"audio";

/// 控制密钥 HKDF info。
pub const CONTROL_KEY_INFO: &[u8] = b"control";

/// 默认 stream_id（loopback 自测用）。
pub const DEFAULT_STREAM_ID: u32 = 1;

/// 单帧 PCM 样本总数（交错，Int16 或 f32）= 480 * 2。
pub const FRAME_SAMPLES_TOTAL: usize = SAMPLES_PER_FRAME_PER_CHANNEL * CHANNELS as usize;
