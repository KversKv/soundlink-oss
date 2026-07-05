// network/udp_receiver.rs — 占位
//
// 职责：tokio UDP 监听音频端口，接收 AudioPacket，AEAD 解密+校验，
// 按 sequence 重排、丢弃过期包，投递到 audio::jitter_buffer。
