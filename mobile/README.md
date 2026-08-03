# mobile — 移动发送端

移动端职责：采集应用音频 → PCM 归一化(48kHz Stereo) → Opus 编码 → 加密 → 局域网 UDP 发送；主 App 负责发现/配对/引导/状态展示。

## 目录说明

| 目录 | 角色 | 状态 |
|---|---|---|
| `flutter_app/` | **移动端主工程（唯一构建入口）**：Flutter UI + Dart 协议/配对/发现 + 平台通道对接原生采集 | Android ✅ 实测可用；iOS 🟡 待真机验收 |
| `ios/BroadcastExtension/` | iOS ReplayKit Broadcast Upload Extension 的 Swift 采集/编码/发送源码 | 🟡 源码就绪，随 `flutter_app/ios` 工程构建 |
| `android/` | 早期 Android 原生结构参考（Kotlin 采集/编码/发送草稿），**非构建入口** | 📌 仅参考，不参与构建 |

> 构建移动端一律在 `flutter_app/` 下执行：
> - Android：`flutter build apk --release -t lib/main.dart`
> - iOS：先跑 `ios/scripts/build_opus_xcframework.sh`，再用 Xcode 打开 `ios/Runner.xcworkspace`

## 相关文档

- 架构：`docs/First/02-architecture.md`
- 技术选型：`docs/First/07-tech-stack.md`
- 平台合规（ReplayKit / MediaProjection 限制）：`docs/First/08-platform-notes.md`
- Android 开发环境：`docs/user/04-dev-env-android.md`
- iOS 开发环境：`docs/user/03-dev-env-ios.md`
