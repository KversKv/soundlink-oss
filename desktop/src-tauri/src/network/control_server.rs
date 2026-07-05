//! 控制通道（TCP/WS）：配对/握手/心跳/统计。完整实现在阶段 3。
//! 阶段 1：仅定义消息类型与错误码（供后续接入），不启动服务。

pub use crate::constants::PROTOCOL_VERSION;

/// 控制消息类型字符串。
pub mod msg_type {
    pub const HELLO: &str = "hello";
    pub const HELLO_ACK: &str = "hello_ack";
    pub const PAIR_REQUEST: &str = "pair_request";
    pub const PAIR_RESPONSE: &str = "pair_response";
    pub const STREAM_START: &str = "stream_start";
    pub const STREAM_START_ACK: &str = "stream_start_ack";
    pub const STREAM_STOP: &str = "stream_stop";
    pub const HEARTBEAT: &str = "heartbeat";
    pub const STATS: &str = "stats";
    pub const ERROR: &str = "error";
}

/// 错误码（对齐 spec §4）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ErrorCode {
    Ok = 1000,
    Internal = 1001,
    PairingFailed = 1002,
    VersionMismatch = 1003,
    PairingExpired = 1004,
    PairingLocked = 1005,
    NotTrusted = 1006,
    StreamRejected = 1007,
    DecryptFailed = 1008,
    Timeout = 1009,
}
