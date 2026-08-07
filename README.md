# SoundLink

面向头戴式耳机用户的**局域网音频流转**软件：手机（iOS/Android）音频 → 局域网 → 电脑音频设备；支持电脑到电脑互传。

> 加密传输 · 局域网内闭环 · 无遥测上报 · 核心开源（MIT）+ Pro 增强闭源（open-core）

[English](README.en.md) · [许可证](LICENSE) · [隐私政策](docs/privacy.md) · [贡献指南](CONTRIBUTING.md) · [变更日志](CHANGELOG.md)

---

## 它解决什么问题

手机上的音乐、电影想用桌面的耳机 / 声卡 / 音箱听，但手机直连这些设备并不方便。SoundLink 让手机把正在播放的音频通过局域网发给电脑，由电脑输出到高品质音频设备；一次配对后自动重连。

**适用**：听音乐、看长视频。
**不适用**：游戏、连麦等实时互动（延迟不满足）；短视频可能感知轻微延迟。

---

## 功能矩阵

| 平台 | 角色 | 状态 |
|---|---|---|
| Windows | 接收端（桌面播放） | ✅ 实测可用 |
| Windows | 发送端（WASAPI Loopback） | ✅ 实测可用 |
| Android | 发送端（MediaProjection） | ✅ 实测可用 |
| macOS | 接收端（CoreAudio via cpal） | 🟡 代码就绪，未实测 |
| macOS | 发送端（ScreenCaptureKit） | 🔴 占位，未实装 |
| Linux | 接收端 | 🔴 未实装 |
| iOS | 发送端（ReplayKit） | 🟡 工程就绪，待真机验收 |

> **当前实测通过的组合**：Android 手机 → Windows 电脑、Windows → Windows。其他组合请勿按「可用」预期，欢迎参与验证（见 [贡献指南](CONTRIBUTING.md)）。

音频基线：48 kHz / Stereo / Opus 10 ms / 128 kbps / 默认 Jitter 80 ms。
运行时可调参数：Opus 码率、Jitter 档位、桌面音量（采样率/声道/帧长固定为基线）。

---

## 快速开始（Windows 桌面端）

### 1. 环境要求

- Rust（stable，MSVC 工具链）
- Node.js 20（见 [`desktop/ui/.nvmrc`](desktop/ui/.nvmrc)）
- CMake + C 编译器（vendored libopus 1.5 构建依赖）
- [Tauri 2 CLI 前置依赖](https://tauri.app/start/prerequisites/)（WebView2 Runtime 在 Windows 10+ 默认随附）

### 2. 克隆并构建

```powershell
git clone https://github.com/KversKv/SoundLink.git
cd SoundLink/desktop/ui
npm install                 # 安装前端依赖与本地 Tauri CLI
npm run tauri:build:exe     # 产出免安装 exe：desktop/src-tauri/target/release/soundlink.exe
```

需要 NSIS 安装包时（需全局 `cargo install tauri-cli`）：

```powershell
cd SoundLink/desktop/src-tauri
cargo tauri build --features tauri_app
# 产物在 desktop/src-tauri/target/release/bundle/
```

### 3. 运行开发模式

```powershell
cd SoundLink/desktop/src-tauri
cargo tauri dev --features tauri_app
```

> 生产构建必须启用 `tauri_app` feature，否则 Opus 解码会回退 passthrough 产生噪声。不要用 `cargo build --release --features tauri_app` 直接出 GUI exe（会按开发模式加载 `localhost:1420`）。Feature 矩阵与单测命令详见 [`desktop/README.md`](desktop/README.md)，完整打包说明见 [`docs/user/05-build.md`](docs/user/05-build.md)。

---

## 快速开始（Android 发送端）

```bash
cd mobile/flutter_app
flutter pub get
flutter build apk --release -t lib/main.dart
# 产物在 build/app/outputs/flutter-apk/
```

要求：Flutter SDK、Android SDK（minSdk 29）、NDK + CMake（编译 libopus JNI）。详见 [`docs/user/04-dev-env-android.md`](docs/user/04-dev-env-android.md)。

iOS 需 macOS + Xcode，并先执行 [`mobile/flutter_app/ios/scripts/build_opus_xcframework.sh`](mobile/flutter_app/ios/scripts/build_opus_xcframework.sh) 生成 libopus XCFramework，详见 [`docs/user/03-dev-env-ios.md`](docs/user/03-dev-env-ios.md)。

---

## 使用方式（终端用户）

1. 电脑端启动 SoundLink，选择输出音频设备，切到 **接收** 角色并开始接收，界面显示 8 位配对码。
2. 手机端打开 App，在设备列表选中电脑（mDNS 自动发现），输入配对码完成配对。
3. Android：点「开始采集」并允许系统弹窗授权；iOS：控制中心长按屏幕录制 → 选 SoundLink → 开始广播。
4. 播放音频即可在电脑端听到；配对信息持久化，下次自动重连。

完整操作手册见 [`docs/user/07-usage.md`](docs/user/07-usage.md)，问题排查见 [`docs/user/08-troubleshooting.md`](docs/user/08-troubleshooting.md)。

---

## 已知限制

- **仅局域网**：不支持公网 / NAT 穿透；路由器开启 AP 隔离或访客网络会阻断。
- **DRM 内容不可采**：WASAPI Loopback / ReplayKit / MediaProjection 受系统 DRM 策略约束，部分流媒体音频会静音，SoundLink 无法也不试图绕过。
- **无全局虚拟声卡**：iOS/Android 只能采集系统允许被捕获的音频，不做后台静默全量捕获。
- **延迟**：面向听音乐 / 看视频；建议电脑端使用有线 / USB / 2.4G 低延迟耳机。
- **单接收端**：当前一个发送端对应一个接收端。
- **桌面 UI 仅中文**：英文 i18n 在规划中。
- **安装包未代码签名**：Windows SmartScreen 首次运行会告警，可用 Release 页提供的 SHA256 自行校验。

---

## 免费 vs Pro

SoundLink 采用 **open-core** 模型：

1. **核心音频流转永久免费、完整开源（MIT）**。本仓库自行编译即可得到与官方一致的免费版，功能完整无残缺——音质、延迟、码率、加密、配对全部不设限。
2. **Pro 是自动化与便捷性增强（￥9.99 买断）**，其实现代码不开源，是项目维持开发的方式。Pro 卖的只是「少点几下、不用管它」。
3. **Pro 授权完全离线校验**：不联网、不上传任何信息、无激活服务器（见[隐私政策](docs/privacy.md)）。
4. 一次购买永久有效，**含所有后续版本与后续 Pro 新功能**；同一授权最多 3 台设备，7 日内可退。

| 能力 | 免费 | Pro |
|---|:---:|:---:|
| 手机→电脑 / 电脑↔电脑 音频流转 | ✅ | ✅ |
| 全码率 + 全 Jitter 档位 + 加密 + 配对 | ✅ | ✅ |
| 输出设备 / 音量 / 音频参数 / 状态监控 | ✅ | ✅ |
| 记忆已配对设备 | 1 台 | **8 台** |
| 开机自启 + 启动即自动收/发 | — | ✅ |
| 自动重连上次设备（跨启动） | — | ✅ |
| 多套配置一键切换 | — | ✅ |
| 全局快捷键 / 托盘直控 | — | ✅ |

> 免费版每次手动一键即可完成同样的收发；Pro 让它「开机即出声、不用打开 SoundLink」。
> 官方下载的只有一个版本：未激活时行为完全等同免费版，粘贴授权码即解锁。

---

## 目录结构

```
SoundLink/
├── mobile/             # 移动发送端（Flutter 主 App + 原生采集）
│   ├── flutter_app/    # 移动端主工程（真机构建入口）
│   ├── ios/            # iOS BroadcastExtension Swift 源码
│   └── android/        # 早期 Android 原生结构参考（非构建入口）
├── desktop/            # 桌面端（Tauri 2 + Rust + React/TS）
│   ├── src-tauri/      # Rust 核心（网络/音频/配对/设备/配置/日志）+ Tauri 命令
│   └── ui/             # 前端界面
├── shared/             # 跨端协议与常量（单源）
└── docs/               # 设计文档与用户文档
    ├── First/          # 架构/协议/安全/延迟/选型/合规/阶段/目录/规格/计划
    ├── NewFunctions/   # 发布就绪度与开源发布规划
    ├── user/           # 用户与开发文档
    ├── AI_Memory/      # 会话归档与调试实录
    └── privacy.md      # 隐私政策
```

---

## 文档导航

| 主题 | 入口 |
|---|---|
| 顶层导航 | [`docs/First/SoundLinkStructrue.md`](docs/First/SoundLinkStructrue.md) |
| 架构 | [`docs/First/02-architecture.md`](docs/First/02-architecture.md) |
| 音频链路 | [`docs/First/03-audio-pipeline.md`](docs/First/03-audio-pipeline.md) |
| 协议规格 | [`docs/First/04-protocol.md`](docs/First/04-protocol.md) |
| 配对与安全 | [`docs/First/05-pairing-security.md`](docs/First/05-pairing-security.md) |
| 延迟与体验 | [`docs/First/06-latency-experience.md`](docs/First/06-latency-experience.md) |
| 平台合规 | [`docs/First/08-platform-notes.md`](docs/First/08-platform-notes.md) |
| 实现规格（编码依据） | [`docs/First/11-implementation-spec.md`](docs/First/11-implementation-spec.md) |
| 开发计划与进度 | [`docs/First/12-plan.md`](docs/First/12-plan.md) |
| 发布就绪度 | [`docs/NewFunctions/release-readiness/00-release-overview.md`](docs/NewFunctions/release-readiness/00-release-overview.md) |
| 开源发布待办 | [`docs/NewFunctions/opensource-launch/00-launch-overview.md`](docs/NewFunctions/opensource-launch/00-launch-overview.md) |
| 市场调研与定位 | [`docs/NewFunctions/opensource-launch/01-market-research.md`](docs/NewFunctions/opensource-launch/01-market-research.md) |
| 用户/开发文档索引 | [`docs/user/00-index.md`](docs/user/00-index.md) |
| 桌面端使用指南 | [`docs/user/desktop-guide.md`](docs/user/desktop-guide.md) |
| 隐私政策 | [`docs/privacy.md`](docs/privacy.md) |
| 协作规则 | [`AGENTS.md`](AGENTS.md)、[`.trae/rules/project-rules.md`](.trae/rules/project-rules.md) |

---

## 当前状态

- 阶段 1/3/4 ✅ 完成：桌面接收器 MVP、配对与设备发现、体验优化。
- 阶段 2 移动端 🟡 进行中：Android ✅ 实测可用；iOS 工程就绪，待真机验收。
- 阶段 5 桌面发送端 🟡 进行中：Windows WASAPI Loopback ✅ 实测可用；macOS 采集未实装。
- 发布就绪度：P0 阻塞红线 ✅ 完成、P1 Beta 前补强 ✅ 完成、P2 后续优化 🟡 进行中。
- 尚未发布正式 Release；跨平台补全（macOS/Linux）与 UI i18n 待做（CI 已就绪）。

详细进度以 [`docs/First/12-plan.md`](docs/First/12-plan.md) 为准，发布待办见 [`docs/NewFunctions/opensource-launch/00-launch-overview.md`](docs/NewFunctions/opensource-launch/00-launch-overview.md)。

---

## 技术栈

- 桌面：Tauri 2 + Rust（tokio）+ React/TypeScript
- iOS：Swift + ReplayKit（采集）+ Flutter（主 App）
- Android：Kotlin + MediaProjection（采集）+ Flutter（主 App）
- 编解码：libopus；传输：UDP(音频) + TCP(控制)；加密：ChaCha20-Poly1305 / X25519 / Ed25519 / HKDF-SHA256

---

## 参与贡献

欢迎 Issue 与 PR。开发环境、代码规范、提交约定与验证命令见 [`CONTRIBUTING.md`](CONTRIBUTING.md)；行为准则见 [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md)。

**当前最需要帮助的方向**：macOS/Linux 端验证与实装、iOS 真机验收、多机型 Android 兼容性反馈、i18n 翻译。

安全漏洞请勿走公开 Issue，按 [`SECURITY.md`](SECURITY.md) 私下反馈。

---

## 许可证

[MIT](LICENSE) · Copyright (c) 2026 KversKv

第三方组件许可证详见 [`docs/privacy.md`](docs/privacy.md) §6。

---

## 问题反馈

- Bug / 功能建议：[GitHub Issues](https://github.com/KversKv/SoundLink/issues)
- 使用讨论：[GitHub Discussions](https://github.com/KversKv/SoundLink/discussions)

