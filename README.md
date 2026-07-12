# SoundLink

面向头戴式耳机用户的**局域网音频流转**软件：手机（iOS/Android）音频 → 局域网 → 电脑音频设备；支持电脑到电脑互传。

> 加密传输 · 局域网内闭环 · 无遥测上报 · 跨平台开源

---

## 功能矩阵

| 平台 | 角色 | 状态 |
|---|---|---|
| Windows | 接收端（桌面播放） | ✅ 可用 |
| Windows | 发送端（WASAPI Loopback） | ✅ 可用 |
| macOS | 接收端 | ✅ 可用 |
| macOS | 发送端（ScreenCaptureKit） | 🟡 占位，未实装 |
| Linux | 接收端 | 🟡 未实装 |
| iOS | 发送端（ReplayKit） | 🟡 工程就绪，待真机验收 |
| Android | 发送端（MediaProjection） | ✅ 可用 |

音频基线：48 kHz / Stereo / Opus 10 ms / 128 kbps / 默认 Jitter 80 ms。

---

## 快速开始（Windows 桌面端）

### 1. 环境要求

- Rust（stable，MSVC 工具链）
- Node.js 18+ 与 pnpm/npm
- [Tauri 2 CLI 前置依赖](https://tauri.app/start/prerequisites/)（WebView2 Runtime 在 Windows 10+ 默认随附）

### 2. 克隆并构建

```powershell
git clone https://github.com/KversKv/SoundLink.git
cd SoundLink
cd desktop/src-tauri
cargo build --features tauri_app
cd ..\ui
npm install
npm run build
```

### 3. 运行开发模式

```powershell
cd desktop
npm run tauri dev
```

### 4. 打包发布版本

```powershell
cd desktop
npm run tauri build
# 产物在 desktop/src-tauri/target/release/bundle/
```

详见 [`desktop/README.md`](desktop/README.md)。

---

## 目录结构

```
SoundLink/
├── mobile/             # 移动发送端（iOS + Android，Flutter 主 App + 原生采集）
│   ├── ios/
│   ├── android/
│   └── flutter_app/
├── desktop/            # 桌面端（Tauri 2 + Rust + React/TS）
│   ├── src-tauri/      # Rust 核心（网络/音频/配对/设备/配置/日志）+ Tauri 命令
│   └── ui/             # 前端界面
├── shared/             # 跨端协议与常量
└── docs/               # 设计文档与用户文档
    ├── First/          # 架构/协议/安全/延迟/选型/合规/阶段/目录/规格/计划
    ├── NewFunctions/   # 发布就绪度规划（P0/P1/P2）
    │   └── release-readiness/  # 分级路线图文档
    ├── user/           # 用户与开发文档
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
| 实现规格（编码依据） | [`docs/First/11-implementation-spec.md`](docs/First/11-implementation-spec.md) |
| 开发计划与进度 | [`docs/First/12-plan.md`](docs/First/12-plan.md) |
| 发布就绪度 | [`docs/NewFunctions/release-readiness/00-release-overview.md`](docs/NewFunctions/release-readiness/00-release-overview.md) |
| 用户使用指南 | [`docs/user/desktop-guide.md`](docs/user/desktop-guide.md) |
| 隐私政策 | [`docs/privacy.md`](docs/privacy.md) |
| 协作规则 | [`AGENTS.md`](AGENTS.md)、[`.trae/rules/project-rules.md`](.trae/rules/project-rules.md) |

---

## 当前状态

- 阶段 1/3/4 ✅ 完成：桌面接收器 MVP、配对与设备发现、体验优化。
- 阶段 2 移动端 🟡 进行中：Android ✅ 可用；iOS 工程就绪，待真机验收。
- 阶段 5 桌面发送端 🟡 进行中：Windows WASAPI Loopback ✅；macOS 采集未实装；双电脑真机未验收。
- P0 阻塞发布红线 ✅ 完成（2026-07-12）。
- P1 Beta 前补强 🟡 进行中（参考 [`docs/NewFunctions/release-readiness/02-p1-important-improvements.md`](docs/NewFunctions/release-readiness/02-p1-important-improvements.md)）。

详细进度以 [`docs/First/12-plan.md`](docs/First/12-plan.md) 为准。

---

## 技术栈

- 桌面：Tauri 2 + Rust（tokio）+ React/TypeScript
- iOS：Swift + SwiftUI + ReplayKit
- Android：Kotlin + Compose + MediaProjection
- 编解码：libopus；传输：UDP(音频) + TCP/WS(控制)；加密：ChaCha20-Poly1305 / X25519 / Ed25519 / HKDF-SHA256

---

## 许可证

[MIT](LICENSE) · Copyright (c) 2026 KversKv

第三方组件许可证详见 [`docs/privacy.md`](docs/privacy.md) §6。

---

## 问题反馈

- GitHub Issues：https://github.com/KversKv/SoundLink/issues
