# shared/protocol — 协议定义（单源）

集中定义供各端对齐的协议契约。建议以语言无关格式（如 JSON Schema / Protobuf，可选）描述，或至少在此维护权威说明，各端手写实现须与之一致。

## 控制消息（JSON over TCP/WS）
hello / pair_request / pair_response / stream_start / stream_stop /
heartbeat / stats / control_action / control_action_ack / error

### 通用控制动作

`control_action` 用于非音频生命周期的低频指令，例如媒体键、快捷指令设置与触发。

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

预留动作名：`media.play_pause` / `media.previous` / `media.next` / `shortcut.set` / `shortcut.trigger`。

## 音频包（AudioPacket，二进制）
magic, version, header_len, stream_id, sequence, timestamp, codec,
sample_rate, channels, frame_duration_ms, flags, payload_len, payload, auth_tag

## 错误码
待定义（配对失败/超时/版本不兼容/解密失败等）。

权威说明见 `docs/First/04-protocol.md`。
