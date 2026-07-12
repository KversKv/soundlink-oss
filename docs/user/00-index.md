# SoundLink 用户与开发文档索引

本目录（`docs/user/`）面向**开发者与使用者**，提供环境搭建、调试、编译、使用等实操指南。设计与架构决策请见 [`docs/First/`](../First/SoundLinkStructrue.md)。

> 当前仓库**多平台并行开发**：桌面端 Windows 可用（接收 + WASAPI Loopback 发送）；Android 可用；iOS 工程就绪待真机验收；macOS 发送端采集未实装。本目录文档随各阶段落地持续补全。开发阶段见 [`docs/First/09-roadmap.md`](../First/09-roadmap.md)。

## 文档一览

| 文档 | 内容 |
|---|---|
| [01-dev-env-common.md](./01-dev-env-common.md) | 通用前置：仓库结构、通用工具链、共享层 |
| [02-dev-env-desktop.md](./02-dev-env-desktop.md) | 桌面端（Tauri 2 + Rust）环境搭建（Windows / macOS / Linux） |
| [03-dev-env-ios.md](./03-dev-env-ios.md) | iOS 端环境搭建（macOS + Flutter + Xcode） |
| [04-dev-env-android.md](./04-dev-env-android.md) | Android 端环境搭建（Flutter + Android Studio） |
| [05-build.md](./05-build.md) | 各平台编译 / 打包方式 |
| [06-debug.md](./06-debug.md) | 各平台调试方式与日志 |
| [07-usage.md](./07-usage.md) | 使用操作手册（配对、连接、播放） |
| [08-troubleshooting.md](./08-troubleshooting.md) | 常见问题与排查 |
| [desktop-guide.md](./desktop-guide.md) | 桌面端终端用户使用指南（安装/配对/收发/设置/常见问题/卸载） |

## 平台矩阵速览

| 端 | 开发平台 | 主要工具 |
|---|---|---|
| 桌面（Receiver/Sender） | Windows / macOS / Linux | Rust、Node.js、Tauri CLI |
| iOS 发送端 | 仅 macOS | Flutter（Dart）主 App、Xcode、Swift（采集 Extension） |
| Android 发送端 | Windows / macOS / Linux | Flutter（Dart）主 App、Android Studio、JDK、Kotlin（采集 Service） |
