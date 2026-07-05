# mobile — 移动发送端

- `ios/` — iOS 工程（Swift + SwiftUI + ReplayKit Broadcast Extension）
- `android/` — Android 工程（Kotlin + Compose + MediaProjection）

移动端职责：采集应用音频 → PCM 归一化(48kHz Stereo) → Opus 编码 → 加密 → 局域网 UDP 发送；主 App 负责发现/配对/引导/状态展示。

详见 `docs/First/02-architecture.md`、`docs/First/07-tech-stack.md`、`docs/First/08-platform-notes.md`。
