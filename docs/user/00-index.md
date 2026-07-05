# SoundLink 用户与开发文档索引

本目录（`docs/user/`）面向**开发者与使用者**，提供环境搭建、调试、编译、使用等实操指南。设计与架构决策请见 [`docs/First/`](../First/SoundLinkStructrue.md)。

> 当前仓库处于**骨架 + 占位**阶段，尚未完成脚手架初始化（Tauri / Xcode / Gradle）。本目录文档描述的是**目标工作流**，随各阶段落地会持续补全，实际命令以脚手架就绪后为准。开发阶段见 [`docs/First/09-roadmap.md`](../First/09-roadmap.md)。

## 文档一览

| 文档 | 内容 |
|---|---|
| [01-dev-env-common.md](./01-dev-env-common.md) | 通用前置：仓库结构、通用工具链、共享层 |
| [02-dev-env-desktop.md](./02-dev-env-desktop.md) | 桌面端（Tauri 2 + Rust）环境搭建（Windows / macOS / Linux） |
| [03-dev-env-ios.md](./03-dev-env-ios.md) | iOS 端环境搭建（macOS + Xcode） |
| [04-dev-env-android.md](./04-dev-env-android.md) | Android 端环境搭建（Windows / macOS / Linux） |
| [05-build.md](./05-build.md) | 各平台编译 / 打包方式 |
| [06-debug.md](./06-debug.md) | 各平台调试方式与日志 |
| [07-usage.md](./07-usage.md) | 使用操作手册（配对、连接、播放） |
| [08-troubleshooting.md](./08-troubleshooting.md) | 常见问题与排查 |

## 平台矩阵速览

| 端 | 开发平台 | 主要工具 |
|---|---|---|
| 桌面（Receiver/Sender） | Windows / macOS / Linux | Rust、Node.js、Tauri CLI |
| iOS 发送端 | 仅 macOS | Xcode、Swift |
| Android 发送端 | Windows / macOS / Linux | Android Studio、JDK、Kotlin |
