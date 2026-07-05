# 11 · 实现规格书（Implementation Spec）

> 本文件把架构落到**可直接编码的精确定义**：字节布局、消息 schema、错误码、握手/加密步骤、状态机、默认常量、脚手架约定、MVP 自测闭环。
> AI/开发者据此生成的各端实现**必须字节级/字段级互通**。与 [04-protocol](./04-protocol.md)、[05-pairing-security](./05-pairing-security.md) 配套；如有冲突以本文件为准并同步回改。

---

## 1. 默认常量（单源，对齐 `shared/constants`）

| 常量 | 值 |
|---|---|
| mDNS 服务类型 | `_soundlink._udp.local.` |
| 协议版本 `PROTOCOL_VERSION` | `1` (u8) |
| AudioPacket 魔数 `MAGIC` | `0x534C` ("SL", 大端) |
| 默认控制端口 `DEFAULT_CONTROL_PORT` | `47810` (TCP/WS) |
| 默认音频端口 `DEFAULT_AUDIO_PORT` | `47811` (UDP) |
| 采样率 | `48000` |
| 声道 | `2` |
| 采样格式（内部） | `Int16` 小端交错 (L,R,L,R…) |
| 编码 | Opus, `codec=1` |
| Opus 帧长 | `10 ms`（每帧 480 样本/声道） |
| Opus 起始码率 | `128000 bps` |
| 默认 Jitter | `80 ms`（低40/平衡80/稳150） |
| 配对码 | 8 位数字，有效期 `120 s`，尝试 `5` 次 |
| AEAD | ChaCha20-Poly1305（key 32B, nonce 12B, tag 16B） |
| 心跳间隔 / 超时 | `2 s` / `6 s` |

> 字节序：AudioPacket 二进制头**统一大端（Big-Endian / network order）**；音频 PCM 样本本身小端。

---

## 2. AudioPacket 精确字节布局（UDP 载荷）

固定头部 **32 字节**，随后为加密载荷与认证标签。所有多字节整数**大端**。

| 偏移 | 字段 | 类型 | 大小(B) | 说明 |
|---:|---|---|---:|---|
| 0  | magic | u16 | 2 | `0x534C` |
| 2  | version | u8 | 1 | `1` |
| 3  | header_len | u8 | 1 | 固定 `32` |
| 4  | stream_id | u32 | 4 | 会话内流标识 |
| 8  | sequence | u32 | 4 | 从 0 递增，回绕按 u32 |
| 12 | timestamp | u64 | 8 | 采样计数（每帧 +480/声道，即 +480） |
| 20 | codec | u8 | 1 | `1`=opus |
| 21 | channels | u8 | 1 | `2` |
| 22 | frame_duration_ms | u8 | 1 | `10` |
| 23 | flags | u8 | 1 | bit0=stream_end，其余保留 |
| 24 | sample_rate | u32 | 4 | `48000` |
| 28 | payload_len | u16 | 2 | 密文长度（不含 tag） |
| 30 | reserved | u16 | 2 | `0` |
| 32 | payload | bytes | payload_len | ChaCha20-Poly1305 密文（明文=Opus 帧） |
| 32+payload_len | auth_tag | bytes | 16 | AEAD tag |

- **AEAD 参数**：
  - `key` = 会话音频密钥（见 §5）。
  - `nonce`(12B) = `stream_id`(4B, BE) ‖ `sequence`(4B, BE) ‖ `0x00000000`(4B)。保证同一会话内 nonce 唯一。
  - `AAD`（关联数据）= AudioPacket 前 32 字节头部原文。
- 接收端：校验 magic/version/header_len → 用头部作 AAD 解密 payload → 得到 Opus 帧。校验失败即丢弃。

### 参考编码伪码
```text
buf = BE.u16(MAGIC) ‖ u8(1) ‖ u8(32) ‖ BE.u32(stream_id)
    ‖ BE.u32(sequence) ‖ BE.u64(timestamp) ‖ u8(1) ‖ u8(2)
    ‖ u8(10) ‖ u8(flags) ‖ BE.u32(48000) ‖ BE.u16(payload_len) ‖ BE.u16(0)
header = buf[0..32]
nonce  = stream_id_be ‖ sequence_be ‖ 00000000
(cipher, tag) = ChaCha20Poly1305(key, nonce, aad=header, plaintext=opus_frame)
packet = header ‖ cipher ‖ tag
```

---

## 3. 控制协议（JSON，换行分帧 / WebSocket text）

传输：TCP 时每条消息一行（`\n` 结尾）的 UTF-8 JSON；WebSocket 时每条 text 帧一条。所有消息含公共字段 `type`、`msg_id`(客户端生成的字符串)、`ts`(unix ms)。

### 3.1 hello  (Sender→Receiver)
```json
{
  "type": "hello",
  "msg_id": "c-1",
  "ts": 1730000000000,
  "protocol_version": 1,
  "device_id": "ios-ab12cd",
  "device_name": "My iPhone",
  "role": "sender",
  "platform": "ios",
  "capabilities": { "codec": ["opus"], "sample_rate": 48000, "channels": 2 }
}
```

### 3.2 hello_ack  (Receiver→Sender)
```json
{
  "type": "hello_ack",
  "msg_id": "s-1",
  "ts": 1730000000010,
  "protocol_version": 1,
  "device_id": "pc-77aa",
  "device_name": "Bedroom PC",
  "pairing_required": true,
  "trusted": false
}
```

### 3.3 pair_request  (Sender→Receiver)
携带 X25519 公钥与配对码证明（见 §5）。
```json
{
  "type": "pair_request",
  "msg_id": "c-2",
  "ts": 1730000000100,
  "device_id": "ios-ab12cd",
  "sender_pub": "base64(X25519_pub, 32B)",
  "sender_identity_pub": "base64(Ed25519_pub, 32B)",
  "proof": "base64(HMAC-SHA256(pairing_secret, transcript), 32B)"
}
```

### 3.4 pair_response  (Receiver→Sender)
```json
{
  "type": "pair_response",
  "msg_id": "s-2",
  "ts": 1730000000110,
  "result": "ok",
  "receiver_pub": "base64(X25519_pub, 32B)",
  "receiver_identity_pub": "base64(Ed25519_pub, 32B)",
  "proof": "base64(HMAC-SHA256(pairing_secret, transcript'), 32B)"
}
```
失败：`{"type":"pair_response","result":"error","error":{"code":1002,"message":"bad pairing code"}}`

### 3.5 stream_start  (Sender→Receiver)
```json
{
  "type": "stream_start",
  "msg_id": "c-3",
  "ts": 1730000001000,
  "stream_id": 1,
  "audio_port": 47811,
  "codec": "opus",
  "sample_rate": 48000,
  "channels": 2,
  "frame_duration_ms": 10,
  "bitrate": 128000
}
```
回 `stream_start_ack`：`{"type":"stream_start_ack","stream_id":1,"result":"ok","receiver_audio_port":47811}`

### 3.6 stream_stop  (Sender→Receiver)
```json
{ "type": "stream_stop", "msg_id": "c-4", "ts": 1730000050000, "stream_id": 1 }
```

### 3.7 heartbeat  (双向)
```json
{ "type": "heartbeat", "msg_id": "c-5", "ts": 1730000002000 }
```

### 3.8 stats  (Sender→Receiver，周期上报，可选)
```json
{
  "type": "stats", "msg_id": "c-6", "ts": 1730000003000,
  "stream_id": 1,
  "packets_sent": 3000, "bitrate": 128000, "encode_ms_avg": 6.2
}
```
接收端也可回传 `stats`：`packets_recv / packets_lost / jitter_ms / buffer_ms / est_latency_ms`。

### 3.9 error  (双向)
```json
{ "type": "error", "msg_id": "x-1", "ts": 1730000004000,
  "error": { "code": 1003, "message": "protocol version mismatch" } }
```

---

## 4. 错误码枚举

| code | 名称 | 含义 |
|---:|---|---|
| 1000 | OK | 成功（非错误，占位） |
| 1001 | INTERNAL | 内部错误 |
| 1002 | PAIRING_FAILED | 配对码错误/证明校验失败 |
| 1003 | VERSION_MISMATCH | 协议版本不兼容 |
| 1004 | PAIRING_EXPIRED | 配对码过期 |
| 1005 | PAIRING_LOCKED | 尝试次数超限 |
| 1006 | NOT_TRUSTED | 未配对设备直接请求流 |
| 1007 | STREAM_REJECTED | 音频参数不支持 |
| 1008 | DECRYPT_FAILED | AEAD 校验失败 |
| 1009 | TIMEOUT | 心跳/握手超时 |

---

## 5. 配对与密钥（第一版：X25519 + HMAC，可自测）

**配对码派生秘密**：
```text
pairing_secret = HKDF-SHA256(
    ikm  = utf8(pairing_code),        // 8 位数字
    salt = utf8("soundlink-pair-v1"),
    info = utf8(receiver_device_id),
    len  = 32)
```

**证明（防中间人）**：
```text
transcript  = sender_pub ‖ receiver_device_id ‖ protocol_version   // Sender 侧
proof       = HMAC-SHA256(pairing_secret, transcript)
transcript' = receiver_pub ‖ sender_pub ‖ receiver_device_id       // Receiver 侧回证
```
双方各自校验对端 proof；不符 → `PAIRING_FAILED`。

**会话密钥**：
```text
shared = X25519(own_priv, peer_pub)
session_master = HKDF-SHA256(ikm=shared, salt=pairing_secret, info="soundlink-session-v1", len=32)
audio_key      = HKDF-SHA256(ikm=session_master, salt="", info="audio", len=32)   // 用于 AudioPacket AEAD
control_key    = HKDF-SHA256(ikm=session_master, salt="", info="control", len=32) // 后续控制面加密（第一版控制面可明文，仅局域网）
```

**信任持久化**：配对成功后保存对端 `identity_pub`(Ed25519) 与 `device_id`；下次连接 `hello.trusted=true`，跳过配对码，直接 X25519 会话协商并用 identity 签名校验（第一版可用已存 identity_pub 简单校验，签名握手列为后续）。

> 第一版控制面可明文（纯局域网），**音频面必须加密**。升级到 SPAKE2/SRP + 控制面加密见 [05-pairing-security](./05-pairing-security.md)。

---

## 6. 状态机

### 6.1 连接（Sender 视角）
```text
DISCONNECTED
  → (发现/选择设备) → CONNECTING (建立控制连接)
  → hello / hello_ack → CONNECTED
  → 若 pairing_required 且 !trusted → PAIRING
  → pair_request/pair_response ok → PAIRED
  → stream_start/ack → STREAMING (开始发 UDP 音频)
  → stream_stop / 断线 → CONNECTED / DISCONNECTED
错误/超时 → ERROR → DISCONNECTED
```

### 6.2 Receiver 视角
```text
IDLE (mDNS 广播中, 显示配对码)
  → 收到 hello → HANDSHAKING
  → (需配对) 校验 pair_request → PAIRED / 拒绝
  → stream_start_ack → RECEIVING (UDP 收包→jitter→解码→输出)
  → stream_stop / 心跳超时 → IDLE
```

---

## 7. Jitter Buffer 与时钟（可实现的最小规则）

- 目标缓冲 = 模式值（默认 80ms = 8 帧）。按 `timestamp` 排序入队。
- 播放拉取：每 10ms 取一帧；缺帧则调用 Opus PLC 生成一帧（`decode(null)`）。
- 过期包（`sequence` 早于已播放水位）直接丢弃。
- 时钟漂移（阶段 4）：缓冲长期偏高/偏低时，按 ±0.5% 微调重采样比率；第一版可先不做，仅记录 `buffer_ms`。

---

## 8. 各端脚手架约定（最小可编译目标）

### 8.1 桌面（阶段 1 起点）
- 初始化：`npm create tauri-app@latest`（React+TS 模板）到 `desktop/`，Rust 源整理进 `src-tauri/src/` 现有模块结构。
- `src-tauri/Cargo.toml` 关键依赖：`tokio`(rt-multi-thread,net,sync,time)、`tracing`+`tracing-subscriber`、`serde`+`serde_json`、`mdns-sd`、`chacha20poly1305`、`x25519-dalek`、`ed25519-dalek`、`hkdf`+`sha2`+`hmac`、`opus`(或 `audiopus`)、`cpal`（跨平台输出，先统一用 cpal 起步，WASAPI/CoreAudio 专用后端后续替换）、`rubato`（重采样，后续）。
- Tauri commands（`commands/mod.rs`）最小集：`start_receiver()`、`stop_receiver()`、`get_pairing_code()`、`list_output_devices()`、`select_output_device(id)`、`get_status()`。
- 事件推送：Rust → 前端 emit `status`、`stats`、`pairing`。

### 8.2 iOS（阶段 2）
- Xcode workspace：`MainApp` target + `BroadcastExtension`(Broadcast Upload Extension) target + `Shared` framework；启用 App Group `group.com.soundlink`。
- 依赖：libopus（SwiftPM/xcframework）；加密用 CryptoKit（ChaCha20-Poly1305/HKDF/Curve25519）。

### 8.3 Android（阶段 2）
- Gradle 单 module `app`；`minSdk 29`（AudioPlaybackCapture）。
- 前台服务 `AudioCaptureService`（`foregroundServiceType="mediaProjection"`）+ 通知；权限 `FOREGROUND_SERVICE`、`FOREGROUND_SERVICE_MEDIA_PROJECTION`、`INTERNET`。
- 依赖：Opus（`io.github.jaredmdobson` 或 JNI 封装）；加密用 Tink 或 BouncyCastle。

---

## 9. MVP 自测闭环（让 demo 可独立跑起来）

为在没有手机时就能验证桌面端，定义**环回自测**：

1. `desktop/src-tauri` 提供一个 `--selftest` 或独立 `examples/loopback_sender.rs`：
   - 读取本机 `get_pairing_code()`，本地完成 §5 握手（同进程模拟 Sender）。
   - 生成测试音（如 440Hz 正弦）→ Opus 编码 → 按 §2 打包加密 → UDP 发到 `127.0.0.1:47811`。
2. Receiver 正常收包 → jitter → 解码 → 输出到默认设备。
3. **验收**：能听到连续 440Hz 音；`get_status()` 显示 `RECEIVING`、`packets_lost≈0`。

后续 iOS/Android 接入后，替换 loopback sender 为真实设备即可，协议不变。

---

## 10. 目录与文件到规格的映射

| 规格章节 | 落地文件 |
|---|---|
| §1 常量 | `shared/constants`、各端常量定义 |
| §2 AudioPacket | `desktop/.../network/packet.rs`、iOS `UdpAudioSender.swift`、Android `UdpAudioSender.kt` |
| §3 控制消息 | `shared/protocol`、`desktop/.../network/control_server.rs` |
| §4 错误码 | `shared/protocol` |
| §5 配对/密钥 | 各端 `pairing/*`、`crypto/*` |
| §6 状态机 | `control_server.rs`、移动端 pairing/连接管理 |
| §7 Jitter | `desktop/.../audio/jitter_buffer.rs`、`resampler.rs` |
| §9 自测 | `desktop/src-tauri/examples/loopback_sender.rs` |
