# 06 · 调试（DEBUG）方式

各端调试方法与日志查看。环境搭建见对应平台文档。

> 日志红线：**密钥 / 配对码禁止明文落日志**；Rust 核心逻辑使用 `tracing`，禁用 `println!`。

## 1. 桌面端（Tauri 2 + Rust）

### Rust 核心

- 使用 `tracing` 输出日志，通过环境变量控制级别：

```bash
# Windows PowerShell
$env:RUST_LOG="debug"; cargo tauri dev
# macOS / Linux
RUST_LOG=debug cargo tauri dev
```

- 断点调试：用 VS Code + `CodeLLDB` 或 CLion 调试 `desktop/src-tauri` 的 Rust target。
- 网络 / 音频问题重点看 [`network/`](../../desktop/src-tauri/src/network) 与 [`audio/`](../../desktop/src-tauri/src/audio) 模块日志（UDP 接收、Jitter Buffer、Opus 解码、设备输出）。

### 前端（React/TS）

- 开发模式下右键 → **Inspect** 打开 WebView DevTools（需在 Tauri 配置开启 devtools）。
- 前端控制台看 UI 状态与 Tauri command 调用。

### 前后端交互

- Tauri command 位于 [`commands/`](../../desktop/src-tauri/src/commands)；前端调用失败时对照 Rust 侧 `tracing` 日志定位。

## 2. iOS

### 主 App 调试（Flutter）

- `flutter run -d <ios-device-id>` 运行到真机，支持热重载（r）/ 热重启（R）。
- Dart 侧断点与日志用 VS Code / Android Studio 的 Flutter 调试器，或 `flutter logs`。
- 需要原生宿主层断点时，用 Xcode 打开 `Runner.xcworkspace`，日志用 `os_log` / `Logger`（Unified Logging），在 Xcode Console 或 **Console.app** 查看。

### Broadcast Extension 调试（原生 Swift，重点）

Extension 是独立进程且**不含 Flutter 引擎**，调试方式：

1. 先运行主 App（`flutter run` 或 Xcode Run `Runner`）。
2. 在 Xcode **Debug → Attach to Process by PID or Name**，选择 Broadcast Extension 进程；或在 scheme 中设置 Extension 为可调试。
3. 通过控制中心开启广播后，Extension 进程启动，断点生效。
4. Extension 有**内存 / 生命周期限制**，避免在其中放重逻辑（禁嵌 Flutter 引擎），崩溃多为内存超限，优先查缓存与依赖。

- Extension 与主 App 通过 App Group 共享配置/状态；配对状态异常时查共享容器读取逻辑。

## 3. Android

### 主 App 调试（Flutter）

- `flutter run -d <android-device-id>` 运行到真机，支持热重载。
- Dart 侧断点用 IDE Flutter 调试器；原生宿主层断点用 Android Studio。
- 日志用 `flutter logs` 或 `Logcat`：

```bash
adb logcat -s SoundLink:V
```

### 前台采集 Service（原生 Kotlin）

- 采集 Service 为独立组件（不含 Flutter），确认通知栏出现采集通知代表 Service 存活。
- 主 App 经 Platform Channel 调起 Service；调起失败查 channel 名称与原生注册。
- MediaProjection 授权失败 / 无声：查 `capture/AudioCaptureService.kt` 日志与授权弹窗结果。
- 采集不到目标应用音频：多为目标应用禁止被捕获，属预期限制，见 [`docs/First/08-platform-notes.md`](../First/08-platform-notes.md)。

## 4. 端到端联调（音频链路）

1. 桌面端启动 Receiver，确认监听 UDP 端口并显示配对码。
2. 用抓包（Wireshark）确认音频 UDP 包是否到达桌面。
3. 桌面收到但无声：查 Opus 解码 → Jitter Buffer → 设备输出各级日志。
4. 卡顿 / 断续：观察丢包与 Jitter Buffer 状态，参考 [`docs/First/06-latency-experience.md`](../First/06-latency-experience.md)。

常见问题排查见 [08-troubleshooting.md](./08-troubleshooting.md)。
