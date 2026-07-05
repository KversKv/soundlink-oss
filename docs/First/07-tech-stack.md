# 07 · 技术选型（Tech Stack）

## 1. iOS

| 模块 | 技术 |
|---|---|
| 语言 | Swift |
| UI | SwiftUI（必要时混合 UIKit） |
| 系统音频采集 | ReplayKit Broadcast Upload Extension |
| 音频样本处理 | CoreMedia / AVFoundation / AudioToolbox |
| 编码 | libopus |
| 网络 | Network.framework / BSD UDP Socket |
| 发现 | Bonjour / mDNS |
| 主 App ↔ Extension 共享 | App Groups |
| 密钥存储 | Keychain |
| 加密 | ChaCha20-Poly1305 / AES-GCM |
| 上架合规 | 高，基于官方 API |

## 2. Android

| 模块 | 技术 |
|---|---|
| 语言 | Kotlin |
| UI | Jetpack Compose |
| 系统音频采集 | MediaProjection + AudioPlaybackCapture（API 29+） |
| 采集载体 | 前台 Service（`mediaProjection` 类型） |
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
