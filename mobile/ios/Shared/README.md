# Shared — iOS 主 App 与 Extension 共享

- `Protocol/` — AudioPacket / 控制消息编解码（对齐 shared/protocol）
- `Crypto/` — ChaCha20-Poly1305、X25519、密钥派生
- `Models/` — 设备、配对、会话模型
- `Logger/` — 统一日志（禁止打印密钥/配对码）

通过 App Group 在主 App 与 Broadcast Extension 间共享。
