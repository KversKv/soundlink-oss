# 04 · 协议设计（Protocol）

## 1. 通道划分

| 通道 | 传输 | 用途 | 特性 |
|---|---|---|---|
| 控制通道 | TCP / WebSocket | 配对、能力协商、开始/停止、心跳、统计 | 可靠、低频 |
| 数据通道 | UDP 单播 | Opus 音频包 | 低延迟、可丢弃 |

## 2. 服务发现（mDNS）

桌面端广播服务类型：

```text
_soundlink._udp.local
```

TXT 记录示例：

```json
{
  "device_id": "pc-xxxx",
  "device_name": "Bedroom PC",
  "role": "receiver",
  "protocol_version": "1.0",
  "pairing_required": true,
  "audio_codec": "opus",
  "sample_rate": 48000,
  "control_port": 47810,
  "audio_port": 47811
}
```

兜底：手动输入 IP、二维码、UDP 广播发现。

## 3. 控制协议消息（JSON over TCP/WS）

| 消息 | 方向 | 说明 |
|---|---|---|
| `hello` | Sender→Receiver | 声明设备身份、协议版本、能力 |
| `pair_request` | Sender→Receiver | 携带配对码派生的证明 |
| `pair_response` | Receiver→Sender | 密钥协商结果、会话参数 |
| `stream_start` | Sender→Receiver | 声明音频参数、UDP 端口 |
| `stream_stop` | Sender→Receiver | 结束流 |
| `heartbeat` | 双向 | 保活 |
| `stats` | 双向 | 丢包/抖动/延迟统计 |
| `control_action` | 双向 | 通用低频控制动作，如媒体键、快捷指令设置/触发 |
| `control_action_ack` | 双向 | 通用控制动作回执 |
| `error` | 双向 | 错误码与描述 |

`stream_start` / `stream_stop` 只表达音频流生命周期；媒体控制、快捷指令等扩展能力统一走 `control_action`：

```json
{
  "type": "control_action",
  "msg_id": "c-action-1",
  "ts": 1730000005000,
  "action": "media.play_pause",
  "target": "receiver",
  "correlation_id": "optional-id",
  "payload": {}
}
```

回执：

```json
{
  "type": "control_action_ack",
  "msg_id": "s-action-1",
  "ts": 1730000005010,
  "reply_to": "c-action-1",
  "action": "media.play_pause",
  "result": "accepted"
}
```

预留动作名：`media.play_pause` / `media.previous` / `media.next` / `shortcut.set` / `shortcut.trigger` / `audio.params.update` / `audio.params.probe_request` / `audio.params.probe_result`。

音频参数同步走 `control_action`，用于运行中低频协商：

```json
{
  "type": "control_action",
  "msg_id": "c-audio-1",
  "ts": 1730000006000,
  "action": "audio.params.update",
  "target": "receiver",
  "payload": {
    "sample_rate": 48000,
    "channels": 2,
    "frame_duration_ms": 10,
    "bitrate": 128000,
    "jitter_mode": "balanced"
  }
}
```

`jitter_mode` 可运行时立即生效；`bitrate` 由发送端后续编码应用；采样率、声道、帧长在第一版中允许持久化与下次流开始生效，运行中变更的回执应通过 `error.restart_required=true` 提示需重启流或下次开始流。

> 具体字段与错误码在实现阶段落到 `shared/protocol` 定义（建议同源生成各端类型）。

## 4. 音频包格式（RTP-like 二进制）

```text
AudioPacket {
  magic:              u16    // 固定魔数，快速识别
  version:            u8     // 协议版本
  header_len:         u8     // 头部长度
  stream_id:          u32    // 流标识
  sequence:           u32    // 递增序列号，用于重排/丢包检测
  timestamp:          u64    // 采样时间戳（时钟校正）
  codec:              u8     // 编码类型 (opus=1)
  sample_rate:        u32    // 采样率
  channels:           u8     // 声道数
  frame_duration_ms:  u8     // 帧时长
  flags:              u8     // 标志位（bit0=stream_end，bit1=probe 探测包）
  payload_len:        u16    // 加密载荷长度
  payload:            bytes  // 加密后的 Opus 数据
  auth_tag:           bytes  // AEAD 认证标签
}
```

### 设计要点
- `sequence` + `timestamp` 支撑重排、丢包检测、Jitter Buffer、时钟校正。
- `payload` 为 **加密后** 的 Opus 数据；`auth_tag` 做完整性校验。
- 头部尽量小，减少每 10ms 包的开销。

> **精确字节布局、AEAD nonce/AAD 定义见 [11-implementation-spec §2](./11-implementation-spec.md#2-audiopacket-精确字节布局udp-载荷)。本表仅为概览。**

## 5. 为什么第一版不用 WebRTC

| 维度 | 自研 Opus+UDP | WebRTC |
|---|---|---|
| 局域网适配 | 契合 | 偏重（NAT 穿透用不上） |
| iOS Extension 集成 | 轻量 | 复杂 |
| 输出到指定桌面设备 | 自己控制 | 仍需自己处理 |
| 调试复杂度 | 低 | 高 |

> 第一版：自研轻量协议。若团队缺音频网络经验，第二选择为 WebRTC media pipeline。

## 6. 端口与网络

- 控制端口、音频端口在发现 TXT 中声明，避免硬编码冲突。
- 仅局域网单播，不做公网/中继。
