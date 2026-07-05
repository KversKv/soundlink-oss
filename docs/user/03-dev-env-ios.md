# 03 · 开发环境搭建 · iOS 发送端

iOS 端为音频发送端，基于 **Swift + SwiftUI + ReplayKit Broadcast Upload Extension**。采集能力仅使用官方 ReplayKit，**不使用私有 API / 越狱**。

> iOS 开发**仅能在 macOS 上进行**。先完成 [01-dev-env-common.md](./01-dev-env-common.md) 的通用前置。

## 1. 环境要求

| 依赖 | 说明 |
|---|---|
| macOS | 近版本，满足目标 Xcode 要求 |
| Xcode | 从 App Store 安装最新稳定版 |
| Apple Developer 账号 | 真机调试与广播 Extension 需要签名 |
| 真机 iPhone / iPad | ReplayKit 广播需真机，模拟器不支持系统音频采集 |

安装 Xcode 命令行工具：

```bash
xcode-select --install
```

## 2. 工程结构

iOS 工程位于 [`mobile/ios`](../../mobile/ios)：

- `MainApp/` — 主 App（配对、发现、设置、广播引导 UI）
- `BroadcastExtension/` — ReplayKit 广播上传扩展（音频采集 + Opus 编码 + UDP 发送），必须保持**轻量**：无复杂 UI、无大缓存、无重依赖（禁 WebRTC）。
- `Shared/` — 主 App 与 Extension 共享的协议 / 加密 / 模型代码。

## 3. 关键配置

- **App Groups**：主 App 与 Broadcast Extension 通过 App Group 共享配对状态（见 `PairingStateReader.swift`）。需在两个 target 的 Signing & Capabilities 中启用同一 App Group。
- **Keychain**：密钥存储。
- **Bonjour / mDNS**：需在 `Info.plist` 声明 `NSLocalNetworkUsageDescription` 与 `NSBonjourServices`。
- **libopus**：通过 SwiftPM / CocoaPods / 预编译 XCFramework 引入（脚手架阶段确定）。

## 4. 打开工程

> 脚手架就绪后（`.xcodeproj` / `.xcworkspace` 生成）：

```bash
open mobile/ios/SoundLink.xcworkspace   # 或 .xcodeproj
```

在 Xcode 中：
1. 为 `MainApp` 与 `BroadcastExtension` 两个 target 配置签名团队（Team）。
2. 启用相同的 App Group。
3. 选择真机，Run。

## 5. 使用 ReplayKit 广播（开发验证）

采集需用户手动开启：**控制中心 → 屏幕录制（长按）→ 选择 SoundLink 的 Broadcast → 开始广播**。主 App 需提供引导（`BroadcastGuide/`）。

合规提示：DRM / 受保护内容 / 系统通话音频可能无法采集，见 [`docs/First/08-platform-notes.md`](../First/08-platform-notes.md)。

## 6. 编译 / 调试

- 编译打包见 [05-build.md](./05-build.md)。
- 调试（含 Extension 调试、日志）见 [06-debug.md](./06-debug.md)。
