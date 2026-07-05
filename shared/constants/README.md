# shared/constants — 常量（单源）

> 权威值见 `docs/First/11-implementation-spec.md` §1。各端实现须与下表一致，勿散落魔法值。

| 常量 | 值 |
|---|---|
| mDNS 服务类型 | `_soundlink._udp.local.` |
| 协议版本 `PROTOCOL_VERSION` | `1` (u8) |
| AudioPacket 魔数 `MAGIC` | `0x534C` ("SL"，大端) |
| 默认控制端口 `DEFAULT_CONTROL_PORT` | `47810` (TCP/WS) |
| 默认音频端口 `DEFAULT_AUDIO_PORT` | `47811` (UDP) |
| 采样率 | `48000` |
| 声道 | `2` |
| 采样格式（内部） | Int16 小端交错 (L,R,L,R…) |
| 编码 | Opus，`codec=1` |
| Opus 帧长 | `10 ms`（每帧 480 样本/声道） |
| Opus 起始码率 | `128000 bps` |
| 默认 Jitter | `80 ms`（低 40 / 平衡 80 / 稳 150） |
| 配对码 | 8 位数字，有效期 `120 s`，尝试 `5` 次 |
| AEAD | ChaCha20-Poly1305（key 32B, nonce 12B, tag 16B） |
| 心跳间隔 / 超时 | `2 s` / `6 s` |

## 字节序

- AudioPacket 二进制头：**大端（network order）**。
- 音频 PCM 样本：小端。

## HKDF 派生标签

- 配对 salt：`"soundlink-pair-v1"`
- 会话 info：`"soundlink-session-v1"`
- 音频密钥 info：`"audio"`
- 控制密钥 info：`"control"`
