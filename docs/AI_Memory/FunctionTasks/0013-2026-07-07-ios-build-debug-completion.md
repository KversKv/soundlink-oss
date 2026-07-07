<!-- FT-0013 -->
# iOS 编译与真机调试补全实录（2026-07-07）

> 场景：用户在内网 macOS 按 `docs/user/03-dev-env-ios.md` 无法成功编译和真机调试，要求推进 iOS 到可编译/可调试闭环，并补充面向首次 macOS 开发者的详细步骤。

## 背景

- iOS 采用 Flutter 主 App + Swift ReplayKit Broadcast Upload Extension。
- 主要阻塞点集中在真机签名、App Group、BroadcastExtension 嵌入、libopus 链接、主 App 到 Extension 的 SessionConfig 字段映射，以及用户文档不够具体。

## 实现清单

| 文件 | 变更 |
|---|---|
| `mobile/flutter_app/ios/scripts/build_opus_xcframework.sh` | 新增本地 libopus iOS XCFramework 生成脚本，输出 `ios/Frameworks/Opus.xcframework` |
| `mobile/flutter_app/ios/Runner.xcodeproj/project.pbxproj` | BroadcastExtension target 接入 Opus.xcframework，新增首次构建自动生成 Opus 的脚本阶段，Runner/BroadcastExtension 补齐签名与 entitlements 关联 |
| `mobile/flutter_app/ios/Runner/Info.plist` | 增加本地网络权限与 `_soundlink._udp` Bonjour 声明 |
| `mobile/flutter_app/ios/Runner/Runner.entitlements` | 保留 App Group entitlement，移除 plist 内注释以降低 Xcode 解析风险 |
| `mobile/flutter_app/ios/Runner/SoundLinkPlugin.swift` | App Group 不可访问时返回明确 FlutterError，便于定位签名/App Group 配置问题 |
| `mobile/ios/BroadcastExtension/PairingStateReader.swift` | 增加 snake_case CodingKeys，匹配 Flutter `SessionConfig.toJson()` 输出 |
| `mobile/ios/BroadcastExtension/OpusEncoderWrapper.swift` | 显式 `import Opus` 并修正 `opus_encoder_create` 参数类型 |
| `mobile/flutter_app/ios/Podfile` | post_install 统一 iOS deployment target 到 13.0 |
| `docs/user/03-dev-env-ios.md` | 重写为 macOS 初学者可执行指南：环境安装、Xcode 按钮路径、签名、App Group、构建、真机调试、ReplayKit 启动和故障排查 |
| `docs/First/12-plan.md` | 同步 iOS 阶段验收进展，标明仍待 macOS/Xcode 真机端到端验收 |

## 关键决策

- 不引入 WebRTC 或重型依赖，继续沿用 ReplayKit + Opus + UDP + ChaCha20-Poly1305。
- libopus 复用仓库内 Android 已引入的源码，通过 macOS/Xcode/CMake 生成 iOS XCFramework，避免额外网络依赖。
- iOS 验收项未勾选完成，因为当前环境是 Windows，无法在本机执行 Xcode 真机编译和 ReplayKit 端到端验证。

## 验证结果

- `flutter analyze`：通过，No issues found。
- IDE diagnostics：无诊断。
- macOS/Xcode 真机编译与 ReplayKit 端到端播放：待用户在 macOS 真机环境执行文档步骤验证。

## 已知边界

- 首次构建需要 macOS 已安装 Xcode、CMake、Flutter、CocoaPods，并能访问 Apple 签名服务。
- App Group 可能因 Apple 账号/Team 权限无法创建默认 `group.com.soundlink`，文档已说明需要统一改为团队可用的 App Group。
- ReplayKit 不保证 DRM、受保护内容、系统通话或所有第三方 App 音频可采集。
