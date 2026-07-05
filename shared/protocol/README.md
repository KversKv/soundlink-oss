# shared/protocol — 协议定义（单源）

集中定义供各端对齐的协议契约。建议以语言无关格式（如 JSON Schema / Protobuf，可选）描述，或至少在此维护权威说明，各端手写实现须与之一致。

## 控制消息（JSON over TCP/WS）
hello / pair_request / pair_response / stream_start / stream_stop /
heartbeat / stats / error

## 音频包（AudioPacket，二进制）
magic, version, header_len, stream_id, sequence, timestamp, codec,
sample_rate, channels, frame_duration_ms, flags, payload_len, payload, auth_tag

## 错误码
待定义（配对失败/超时/版本不兼容/解密失败等）。

权威说明见 `docs/First/04-protocol.md`。
