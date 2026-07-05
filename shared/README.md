# shared — 跨端共享约定

各端（iOS/Android/桌面）应对齐这里的定义，避免各写一份魔法值。

- `protocol/` — 控制消息、AudioPacket 结构、错误码
- `constants/` — 服务类型(`_soundlink._udp.local`)、默认端口、音频参数(48kHz/Stereo/Opus 10ms/128kbps)、Jitter 档位

修改协议须同步更新 `docs/First/04-protocol.md`。
