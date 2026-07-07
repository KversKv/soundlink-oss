<!-- FT-0006 -->
# Android 端到端验证结论与架构文档同步（2026-07-07）

> 场景：用户实测 Android + 电脑端已经可以正常出声，需要把验证结论写入进度表，并核对 `docs/First/02-architecture.md`、`docs/First/07-tech-stack.md` 是否匹配当前代码实现。

## 实现清单

| 文件 | 变更 |
|---|---|
| `docs/First/12-plan.md` | 阶段 2 Android 端到端验收改为完成，备注记录用户实测 Android + 电脑端可正常出声 |
| `docs/First/02-architecture.md` | 将移动主 App 从 SwiftUI/Compose 双端描述修正为 Flutter 主 App + 原生采集组件；控制通道修正为 TCP JSON Lines；桌面输出修正为 cpal 抽象；桌面 Sender 状态修正为 Windows WASAPI 已实现、macOS ScreenCaptureKit 占位 |
| `docs/First/07-tech-stack.md` | 对齐当前技术栈：Dart `multicast_dns`、Dart TCP Socket、Android JNI libopus、移动端 `shared_preferences` 当前信任存储、桌面 libopus_sys/cpal/mdns-sd/本地 JSON |
| `mobile/flutter_app/lib/src/services/platform_service.dart` | 修正注释中 Android 配置写入位置：当前为 SharedPreferences，不是 EncryptedSharedPreferences |

## 关键核对结论

- Android 采集链路当前实现为 Flutter MethodChannel 写入会话配置，Kotlin 前台 Service 获取 MediaProjection 授权后用 AudioPlaybackCapture + AudioRecord 采集 48kHz/Stereo/Int16。
- Android 编码与发送链路为 JNI libopus 编码，BouncyCastle ChaCha20-Poly1305 加密，DatagramSocket UDP 发送 AudioPacket。
- 移动主 App 设备发现使用 Dart `multicast_dns`，控制通道使用 Dart `Socket` 连接桌面 TCP JSON Lines 控制服务。
- 移动端信任存储当前使用 `shared_preferences`，Keychain / Keystore 是后续升级方向。
- 桌面音频输出当前统一使用 `cpal` 抽象，Windows WASAPI Loopback Sender 已实现，macOS ScreenCaptureKit Sender 仍是占位。

## 验证结果

- IDE diagnostics：无诊断问题。
- `flutter analyze`（`mobile/flutter_app`）：通过，`No issues found!`。

## 已知边界

- iOS 真机端到端验收仍未完成，`12-plan.md` 保持未勾选。
- 阶段 5 整体仍未完成，macOS ScreenCaptureKit 与双电脑真机验收仍待后续实现/验证。
