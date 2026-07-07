<!-- FT-0004 -->

# 移动端构建闭环与计划同步（2026-07-07）

> 场景：根据验收建议补齐 Android Gradle/CMake/libopus 构建闭环、iOS BroadcastExtension 工程配置、重复工程清理、Rust feature 构建验证，并同步 docs/First 计划状态。

## 背景

- 目标优先级：P0 Android/iOS 真机端到端准备，P1 修正阶段 5 macOS ScreenCaptureKit 状态与清理重复工程，P2 补 Opus/Tauri GUI/WASAPI feature 构建验证。
- 约束：当前阶段不升级安全相关设计与实现。

## 实现清单

| 类型 | 文件/目录 | 结果 |
|---|---|---|
| Android 构建 | `mobile/flutter_app/android/app/build.gradle.kts` | 合并重复依赖块，保留 BouncyCastle 与 AndroidX 依赖，配置真机 ABI 过滤 |
| Android CMake | `mobile/flutter_app/android/app/src/main/cpp/CMakeLists.txt` | 接入本地 libopus 源码，关闭 x86 SIMD intrinsic 分支，解决 x86_64 AVX2 编译失败 |
| iOS 工程 | `mobile/flutter_app/ios/Runner.xcodeproj/project.pbxproj` | 增加 BroadcastExtension target、Swift 源码引用、Embed App Extensions、构建配置 |
| iOS 配置 | `mobile/flutter_app/ios/BroadcastExtension/Info.plist` | 增加 Broadcast Upload Extension Info.plist |
| iOS 配置 | `mobile/flutter_app/ios/BroadcastExtension/BroadcastExtension.entitlements` | 增加 App Group `group.com.soundlink` |
| iOS Flutter 插件 | `mobile/flutter_app/ios/Runner/SoundLinkPlugin.swift` | Broadcast picker 指向 `com.soundlink.soundlink.BroadcastExtension` |
| 桌面 WASAPI | `desktop/src-tauri/src/audio/capture/wasapi_loopback.rs` | 修正 f32→i16 负满幅映射，`-1.0` 映射为 `-32768` |
| 桌面命令 | `desktop/src-tauri/src/commands/mod.rs` | 保持 WASAPI 条件编译下可变 sources，并避免非 WASAPI 构建告警 |
| 清理 | `mobile/mobile` | 删除重复嵌套 Flutter 工程 |
| 文档 | `docs/First/12-plan.md` | 修正阶段 5/macOS ScreenCaptureKit 状态，补充构建验证与真机待验说明 |
| 文档 | `docs/First/10-project-structure.md` | 同步当前移动端主入口与重复工程清理结果 |
| 文档 | `docs/First/11-implementation-spec.md` | 同步各端脚手架与构建验证基线 |

## 验证结果

- `mobile/flutter_app/android/gradlew clean :app:assembleDebug`：通过。
- `desktop/src-tauri/cargo test --features opus`：通过，真实 libopus roundtrip 覆盖。
- `desktop/src-tauri/cargo test --features wasapi`：通过，50 passed。
- `desktop/src-tauri/cargo check --features tauri_app`：通过。
- `desktop/ui/npm run build`：通过。
- 既有 `flutter analyze` / `flutter test`：此前已通过，本次未改 Flutter Dart 业务逻辑。

## 关键决策

- Android 优先闭合 APK 构建链路，而不是把 x86_64 模拟器 ABI 从构建矩阵中强行移除；通过关闭 libopus x86 SIMD intrinsic 分支提升 debug 构建稳定性。
- iOS 在 Windows 环境仅补齐 Xcode 工程配置与共享配置，不把真机端到端伪标记为完成。
- 阶段 5 改为进行中：Windows WASAPI 与 Sender 抽象已验证，但 macOS ScreenCaptureKit 仍为占位，双电脑真机验收未完成。
- 未进行 PAKE、控制面加密等安全升级，遵守当前新项目阶段约束。

## 待外部环境确认

- Android 真机：MediaProjection 授权、AudioPlaybackCapture 实际采集、UDP 到桌面播放端到端。
- iOS 真机：macOS/Xcode 打包、签名/App Group provisioning、libopus xcframework 链接、ReplayKit BroadcastExtension 授权与端到端播放。
- macOS 桌面发送：ScreenCaptureKit SCStream 采集实现与 macOS 真机验证。

## 关联文档

- `docs/First/12-plan.md`
- `docs/First/10-project-structure.md`
- `docs/First/11-implementation-spec.md`
