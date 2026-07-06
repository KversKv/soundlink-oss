# 03 · 开发环境搭建 · iOS 发送端

iOS 端为音频发送端，采用「**Flutter 主 App + 原生采集 Extension**」分层架构：主 App UI 用 **Flutter（Dart）** 与 Android 共用一套；系统音频采集用 **原生 Swift + ReplayKit Broadcast Upload Extension**。采集能力仅使用官方 ReplayKit，**不使用私有 API / 越狱**。架构决策见 [`docs/First/07-tech-stack.md`](../First/07-tech-stack.md) §6、[`docs/First/08-platform-notes.md`](../First/08-platform-notes.md) §1b。

> iOS 开发**仅能在 macOS 上进行**。先完成 [01-dev-env-common.md](./01-dev-env-common.md) 的通用前置。

## 1. 环境要求

| 依赖 | 说明 |
|---|---|
| macOS | 近版本，满足目标 Xcode 要求 |
| Flutter SDK | 稳定版（含 Dart）；`flutter doctor` 全绿 |
| Xcode | 从 App Store 安装最新稳定版（Flutter iOS 构建与原生 Extension 均需要） |
| CocoaPods | Flutter iOS 依赖管理需要（`sudo gem install cocoapods`） |
| Apple Developer 账号 | 真机调试与广播 Extension 需要签名 |
| 真机 iPhone / iPad | ReplayKit 广播需真机，模拟器不支持系统音频采集 |

安装 Xcode 命令行工具并校验 Flutter：

```bash
xcode-select --install
flutter doctor          # 按提示补齐 iOS 工具链
```

## 2. 工程结构

移动端 Flutter 主 App 位于 [`mobile/flutter_app`](../../mobile/flutter_app)（iOS/Android 共用）；iOS 原生宿主与采集扩展位于其 `ios/` 目录：

- `mobile/flutter_app/lib/` — Flutter 主 App（配对、发现、设置、广播引导 UI，Dart）
- `mobile/flutter_app/ios/Runner/` — iOS 原生宿主（承载 Flutter 引擎 + 与 Extension 桥接）
- `mobile/flutter_app/ios/BroadcastExtension/` — ReplayKit 广播上传扩展（音频采集 + Opus 编码 + UDP 发送，**原生 Swift**）。必须保持**轻量**：不嵌入 Flutter 引擎、无复杂 UI、无大缓存、无重依赖（禁 WebRTC）。
- `mobile/flutter_app/ios/Shared/` — 主 App 宿主与 Extension 共享的协议 / 加密 / 模型代码（Swift）。

> **Flutter 只在主 App 进程**；Broadcast Extension 是独立受限进程，保持纯原生。

## 3. 关键配置

- **App Groups**：Flutter 主 App（Runner）与 Broadcast Extension 通过 App Group 共享配对状态与配置（对端地址、密钥、开关）。需在两个 target 的 Signing & Capabilities 中启用同一 App Group。
- **Keychain**：密钥存储。
- **Bonjour / mDNS**：需在 `Info.plist` 声明 `NSLocalNetworkUsageDescription` 与 `NSBonjourServices`。
- **libopus**：在 Extension 侧通过 SwiftPM / CocoaPods / 预编译 XCFramework 引入（脚手架阶段确定）。

## 4. 打开与运行

> 脚手架就绪后：

主 App 用 Flutter 命令运行（真机）：

```bash
cd mobile/flutter_app
flutter pub get
flutter run -d <ios-device-id>     # flutter devices 查看设备 id
```

需要调试/配置原生 Extension 时，用 Xcode 打开 Flutter 生成的工程：

```bash
open mobile/flutter_app/ios/Runner.xcworkspace
```

在 Xcode 中：
1. 为 `Runner`（主 App）与 `BroadcastExtension` 两个 target 配置签名团队（Team）。
2. 启用相同的 App Group。
3. 选择真机，Run。

## 5. 使用 ReplayKit 广播（开发验证）

采集需用户手动开启：**控制中心 → 屏幕录制（长按）→ 选择 SoundLink 的 Broadcast → 开始广播**。Flutter 主 App 需提供引导页。

合规提示：DRM / 受保护内容 / 系统通话音频可能无法采集，见 [`docs/First/08-platform-notes.md`](../First/08-platform-notes.md)。

## 6. 编译 / 调试

- 编译打包见 [05-build.md](./05-build.md)。
- 调试（含 Flutter 主 App 与原生 Extension 调试、日志）见 [06-debug.md](./06-debug.md)。
