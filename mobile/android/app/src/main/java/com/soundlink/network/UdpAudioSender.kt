package com.soundlink.network

// UdpAudioSender — 占位
//
// 职责：将 Opus 数据打包为 AudioPacket（docs/First/04-protocol.md），
// ChaCha20-Poly1305 加密后经 DatagramSocket UDP 单播发送到桌面端。
// 维护 sequence / timestamp，处理重连。
