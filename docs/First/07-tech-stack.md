# 07 · 技术选型（Tech Stack）

> **移动端架构基调（2026 修订）**：采用「**Flutter 主 App（统一 UI）+ 原生采集组件**」的分层混合架构，而非纯原生双写，亦非纯跨端一套。此决策的动机、边界与业界佐证见本文件 §6。

## 1. iOS

| 模块 | 技术 |
|---|---|
| 语言 | 主 App：Dart（Flutter）；采集 Extension：Swift |
| UI | **Flutter**（主 App 界面：配对/发现/设置/广播引导）；Extension 无 UI |
| 系统音频采集 | ReplayKit Broadcast Upload Extension（**原生 Swift**，不含 Flutter 引擎） |
| 音频样本处理 | CoreMedia / AVFoundation / AudioToolbox |
| 编码 | libopus |
| 网络 | Network.framework / BSD UDP Socket |
| 发现 | Bonjour / mDNS |
| 主 App ↔ Extension 共享 | App Groups（共享容器传递配置/状态；音频包在 Extension 内直接编码发送） |
| 密钥存储 | Keychain |
| 加密 | ChaCha20-Poly1305 / AES-GCM |
| 上架合规 | 高，基于官方 API |

## 2. Android

| 模块 | 技术 |
|---|---|
| 语言 | 主 App：Dart（Flutter）；采集 Service：Kotlin |
| UI | **Flutter**（主 App 界面）；采集前台 Service 无 UI（仅通知栏） |
| 系统音频采集 | MediaProjection + AudioPlaybackCapture（API 29+，**原生 Kotlin**） |
| 采集载体 | 前台 Service（`mediaProjection` 类型，原生实现） |
| PCM 读取 | AudioRecord |
| 编码 | libopus（JNI）或成熟 Opus 封装 |
| 网络 | UDP DatagramSocket + OkHttp/WebSocket 控制 |
| 发现 | NSD (Network Service Discovery) / mDNS |
| 密钥存储 | Android Keystore / EncryptedSharedPreferences |
| 加密 | ChaCha20-Poly1305 |
| 合规 | 需前台通知 + 用户授权屏幕/音频捕获 |

> 说明：`AudioPlaybackCapture` 只能采集**允许被捕获**的应用音频（应用可通过 `allowAudioPlaybackCapture` 声明）；部分应用/受保护内容不可采。

## 3. 桌面端

| 模块 | 技术 |
|---|---|
| 桌面框架 | Tauri 2 |
| 前端 | React + TypeScript |
| 核心 | Rust |
| 异步运行时 | tokio |
| 网络 | tokio UDP + TCP/WebSocket |
| 编解码 | libopus（`opus` / `audiopus` crate 或 FFI） |
| 音频输出 Windows | WASAPI（`IAudioClient3` / `IAudioRenderClient`） |
| 音频输出 macOS | CoreAudio / AudioUnit |
| 音频输出 Linux | PipeWire（后续） |
| 采集（Sender，后续） | WASAPI Loopback / ScreenCaptureKit |
| 发现 | mdns 库 / Bonjour |
| 加密 | ChaCha20-Poly1305 |
| 密钥交换 | X25519（后续 SPAKE2/SRP） |
| 配置存储 | SQLite / 本地 JSON |
| 日志 | tracing |

## 4. 共享层（Shared）

- **协议定义**：控制消息与音频包结构集中定义，尽量单源生成各端类型（如 JSON schema / proto，可选）。
- **常量**：服务类型、端口默认值、音频参数、错误码。

## 5. 选型理由摘要

- **Tauri 2 + Rust**：比 Electron 轻；Rust 适合音频/网络/协议核心，跨平台好，未来核心可复用。
- **自研 Opus + UDP**：局域网低延迟，避免 WebRTC 的重量与 Extension 集成复杂度。
- **官方采集 API（ReplayKit / MediaProjection）**：合规、可上架，不依赖越狱/root/私有 API。
- **移动端 Flutter 主 App**：统一 iOS/Android 界面，消除双端 UI 重复维护（详见 §6）。

## 6. 移动端 UI 架构决策（ADR）

### 背景 / 问题
移动端最初规划为 iOS(SwiftUI) + Android(Compose) **双端分别实现**。实践中，**UI 迭代需两套同步改动**，工作量与走查成本高，是主要维护痛点。底层驱动（采集）差异化可接受。

### 决策
移动端采用 **「Flutter 主 App + 原生采集组件」分层混合架构**：
- **主 App UI（配对、设备发现、设置、广播/授权引导）→ Flutter 统一一套。**
- **系统音频采集（iOS Broadcast Extension / Android 前台 Service）→ 保持各自原生（Swift/Kotlin），不嵌入 Flutter 引擎。**

### 理由（含业界佐证）
- 大厂（微信、抖音、淘宝等）主流做法即 **「原生外壳/受限组件保持原生 + 一致性 UI 用 Flutter + 底层能力沉为跨端内核」的分层混合**，并非纯一套代码全端。SoundLink 规模小、UI 极简，正适合“主界面统一、采集原生”。
- Flutter 自绘引擎（Skia）保证双端 UI 像素级一致，热重载提升迭代效率，生态成熟。
- 采集组件是**受限的独立进程**（iOS Extension 内存/依赖严格受限、Android 前台 Service），**不能也不应**塞入 Flutter 引擎，保持原生轻量符合合规红线（见 08）。

### 边界与代价（务必知晓）
- **“统一”只统一主 App 界面**；采集侧仍是两份原生代码（但其几乎无 UI，本就轻量）。
- 主 App（Flutter）与采集组件（原生）是**跨进程**关系：iOS 经 App Groups 共享容器、Android 经 Service/IPC 传递配置与状态；音频包在采集组件内直接编码并 UDP 发送，不回传主 App。
- 引入 Flutter 属对本文件与 09-roadmap 阶段 2 的**架构级修订**。

### 未被否决的后续方向（暂不排期）
将网络/协议/Opus/加密/配对等纯逻辑下沉为**跨端 Rust 核心库**（桌面已是 Rust，可经 FFI/UniFFI 供 iOS、JNI 供 Android 复用），进一步消除三端逻辑重复。本轮仅确立 UI 框架，共享核心待后续单独评审。
