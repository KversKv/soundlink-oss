// UdpAudioSender.swift — 占位
//
// 职责：将 Opus 数据打包为 AudioPacket（见 docs/First/04-protocol.md），
// 使用会话密钥 ChaCha20-Poly1305 加密，经 UDP 单播发送到已配对桌面端。
// 维护 sequence / timestamp，处理发送失败与重连。
