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

/// 自适应 Jitter 允许的最小/最大目标深度（帧）。
pub const JITTER_AUTO_MIN_FRAMES: usize = 4; // 40ms
pub const JITTER_AUTO_MAX_FRAMES: usize = 20; // 200ms
/// 自适应 Jitter 抖动 EWMA 平滑系数（0..1，越小越平滑）。
pub const JITTER_EWMA_ALPHA: f64 = 0.1;
/// 自适应 Jitter 目标深度系数：target = jitter_ewma * k + base。
pub const JITTER_AUTO_K: f64 = 4.0;
pub const JITTER_AUTO_BASE_FRAMES: usize = 2;

/// 连续 PLC 帧上限：超过后切静音，避免 Opus PLC 持续衰减产生 artifacts。
pub const PLC_CONSECUTIVE_LIMIT: usize = 8;

/// 时钟漂移校正范围（±0.5%，spec §7）。
pub const DRIFT_CORRECTION_MAX_RATIO: f64 = 0.005;
/// 漂移校正目标缓冲水位偏差阈值（帧）：偏差超过此值开始校正。
pub const DRIFT_ADJUST_THRESHOLD_FRAMES: usize = 3;

/// 桌面输出低延迟 buffer（帧数，1 帧 = 10ms）。
pub const OUTPUT_BUFFER_FRAMES: u32 = 2;
/// 桌面输出低延迟 buffer（样本数 = frames * frame_samples_total）。
pub const OUTPUT_BUFFER_SAMPLES: u32 = OUTPUT_BUFFER_FRAMES * FRAME_SAMPLES_TOTAL as u32;

/// 估算端到端延迟的初始值（ms）。
pub const EST_LATENCY_INIT_MS: u32 = 80;
/// stats 上报周期（秒，接收端回传给 sender）。
pub const STATS_REPORT_INTERVAL_SECS: u64 = 1;

/// 弱网丢包率阈值（>此值触发码率下调建议）。
pub const LOSS_RATE_HIGH_THRESHOLD: f64 = 0.05; // 5%
/// 弱网丢包率阈值（<此值触发码率上调建议）。
pub const LOSS_RATE_LOW_THRESHOLD: f64 = 0.01; // 1%
/// 码率建议下限/上限（bps）。
pub const BITRATE_MIN: u32 = 32_000;
pub const BITRATE_MAX: u32 = 192_000;
pub const BITRATE_STEP: u32 = 16_000;

/// 配对码位数。
pub const PAIRING_CODE_DIGITS: usize = 8;

/// 配对码有效期（秒）。
pub const PAIRING_CODE_TTL_SECS: u64 = 120;

/// 配对码最大尝试次数。
pub const PAIRING_CODE_MAX_ATTEMPTS: u32 = 5;

/// 配对码超限锁定时长（秒）。D4：锁定后用户需等待此时长再重试。
pub const PAIRING_LOCK_DURATION_SECS: u64 = 60;

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

// ───────────────────────── 阶段 5：桌面发送端 ─────────────────────────

/// Sender 控制连接超时（秒）。
pub const SENDER_CONNECT_TIMEOUT_SECS: u64 = 5;

/// Sender 心跳间隔（秒，对齐 HEARTBEAT_INTERVAL_SECS）。
pub const SENDER_HEARTBEAT_INTERVAL_SECS: u64 = HEARTBEAT_INTERVAL_SECS;

/// Sender stats 上报周期（秒）。
pub const SENDER_STATS_INTERVAL_SECS: u64 = STATS_REPORT_INTERVAL_SECS;

/// Sender 编码耗时 EWMA 平滑系数。
pub const ENCODE_MS_EWMA_ALPHA: f64 = 0.1;

/// WASAPI loopback 采集线程内部环形缓冲帧数（每帧 10ms）。
pub const CAPTURE_RING_FRAMES: usize = 64;
