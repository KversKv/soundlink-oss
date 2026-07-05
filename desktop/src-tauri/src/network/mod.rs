// network/mod.rs — 占位
pub mod discovery;      // mDNS 广播 _soundlink._udp.local 与发现
pub mod udp_receiver;   // UDP 接收音频包，解密、重排入 jitter buffer
pub mod control_server; // TCP/WebSocket 控制通道：配对/握手/心跳/统计
pub mod packet;         // AudioPacket 编解码（对齐 shared/protocol）
