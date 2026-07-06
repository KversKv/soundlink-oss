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

## 1.5 调试开关（DEBUG / DUMP_ENABLE）

为方便开发期快速联调，各端主入口文件提供两个常量：`DEBUG` 与 `DUMP_ENABLE`（后者默认跟随 `DEBUG`）。**发布前务必改回 `false`。**

### 开关位置

| 端 | 文件 | 默认值 |
|---|---|---|
| 桌面端 | [`desktop/src-tauri/src/main.rs`](../../desktop/src-tauri/src/main.rs) | `pub const DEBUG: bool = false;`<br>`pub const DUMP_ENABLE: bool = DEBUG;` |
| 移动端 | [`mobile/flutter_app/lib/main.dart`](../../mobile/flutter_app/lib/main.dart) | `const bool DEBUG = false;`<br>`const bool DUMP_ENABLE = DEBUG;` |

### DEBUG 模式行为

将 `DEBUG` 改为 `true` 重新编译后：

1. **配对码固定为 `12345678`**
   - 桌面端 [`pairing_code.rs`](../../desktop/src-tauri/src/pairing/pairing_code.rs) 的 `PairingCodeManager::with_debug(true)` 在 `issue()` 时返回固定码（不再随机）。
   - 移动端 [`pairing_page.dart`](../../mobile/flutter_app/lib/src/pages/pairing_page.dart) 的配对码输入框默认填充 `12345678`。
2. **手机端手动添加设备默认填写 `10.31.30.41`**
   - 移动端 [`discovery_page.dart`](../../mobile/flutter_app/lib/src/pages/discovery_page.dart) 的「手动 IP」对话框默认填充该地址，省去手敲。
3. **DUMP_ENABLE 同步开启**（见下）。

### DUMP_ENABLE 功能

控制各客户端是否把音频链路各阶段的 RAW Data 落盘，便于用 Audacity / ffmpeg / Python 分析杂音、错位、丢包等问题。

### 文件保存位置速查

> **关键**：dump 文件**不在仓库根目录**，而是写在各端运行时的工作目录或平台专属沙箱中。下表列出实际位置与访问方式。

| 端 | 角色 | 实际保存目录 | 如何访问 |
|---|---|---|---|
| 桌面（Windows） | 接收器 / 发送器 | `cargo tauri dev` 或 `cargo run` 的**当前工作目录**，通常是 [`desktop/src-tauri/`](../../desktop/src-tauri/) | 资源管理器直接打开 `desktop\src-tauri\` 即可看到 `soundlink_*.bin` / `soundlink_*.raw` |
| 桌面（macOS / Linux） | 接收器 / 发送器 | 同上：启动命令的 cwd | 终端 `ls desktop/src-tauri/soundlink_*` |
| iOS | BroadcastExtension | App Group `group.com.soundlink` 容器内 `soundlink_dump/` 子目录 | Xcode → Window → Devices and Simulators → 选中设备 → 选 App → Download Container，或在 Files App 中查看（需 App 支持）；也可通过 `os_log` 查看路径日志 |
| Android | 采集 Service | 公共下载目录 `Download/soundlink_dump/`（MediaStore，Android 10+ 无需权限），失败回退 app 私有目录 `Android/data/<package>/files/soundlink_dump/` | 系统文件管理器 → Downloads → soundlink_dump；或 `adb shell ls /sdcard/Download/soundlink_dump/`；私有目录用 `adb shell run-as com.soundlink.soundlink ls files/soundlink_dump/` |

> **桌面端 cwd 提示**：Tauri 应用启动时的 cwd 不一定是仓库根。如果你用 IDE Run 按钮，cwd 通常是 `desktop/src-tauri/`；如果用 PowerShell 在仓库根执行 `cargo tauri dev`，cwd 是仓库根。**找不到文件时优先看终端启动路径。**

### 各端转储文件清单

**桌面端接收器**（[`receiver.rs`](../../desktop/src-tauri/src/receiver.rs) 的 `DebugDumper`）写到当前工作目录：

| 文件 | 内容 |
|---|---|
| `soundlink_opus.bin` | 原始 Opus 帧（4 字节小端长度前缀 + 4 字节小端 seq + 数据；丢包占位为 `0xFFFFFFFF` 长度） |
| `soundlink_pcm_decoded.raw` | Opus 解码后 PCM（i16 LE，stereo 交错，48kHz） |
| `soundlink_pcm_resampled.raw` | 漂移校正后 PCM（i16 LE，stereo 交错，送 cpal 前） |

> 仅在收到音频包并触发解码回调时才会写入；只启动 Receiver 不发包不会产生文件。

**桌面端发送器**（[`sender.rs`](../../desktop/src-tauri/src/sender.rs) 的 `send_loop`）写到当前工作目录：

| 文件 | 内容 |
|---|---|
| `soundlink_sender_pcm.raw` | 采集后 PCM（i16 LE，stereo 交错，编码前） |
| `soundlink_sender_opus.bin` | Opus 帧（4 字节小端长度前缀 + 数据） |

> 仅在采集源 `poll_frame()` 实际产出 PCM 时才写入；只握手不启动采集不会产生文件。

**iOS BroadcastExtension**（[`SampleHandler.swift`](../../mobile/ios/BroadcastExtension/SampleHandler.swift)）写到 App Group 共享容器 `soundlink_dump/` 子目录：

| 文件 | 内容 |
|---|---|
| `capture_pcm.raw` | 采集归一化后 PCM（Int16 交错，编码前） |
| `capture_opus.bin` | Opus 帧（4 字节小端长度前缀 + 数据） |

完整路径示例：`<App Group Container>/soundlink_dump/capture_pcm.raw`，其中 App Group 容器路径类似 `/private/var/mobile/Containers/Shared/AppGroup/<UUID>/`。

主 App [`SoundLinkPlugin.swift`](../../mobile/flutter_app/ios/Runner/SoundLinkPlugin.swift) 通过 App Group 键 `soundlink.dump_pcm` 把开关传给 Extension。

**Android 采集 Service**（[`AudioCaptureService.kt`](../../mobile/flutter_app/android/app/src/main/kotlin/com/soundlink/soundlink/capture/AudioCaptureService.kt)）写到公共 `Download/soundlink_dump/`（MediaStore，Android 10+ 无需权限），失败回退 app 私有目录 `getExternalFilesDir(null)/soundlink_dump/`：

| 文件 | 内容 |
|---|---|
| `capture_pcm.raw` | 采集后 PCM（Int16 交错，编码前） |
| `capture_opus.bin` | Opus 帧（4 字节小端长度前缀 + 数据） |

完整路径示例：
- 公共下载目录：`/sdcard/Download/soundlink_dump/capture_pcm.raw`
- 私有目录回退：`/sdcard/Android/data/com.soundlink.soundlink/files/soundlink_dump/capture_pcm.raw`

开关由主 App [`SoundLinkPlugin.kt`](../../mobile/flutter_app/android/app/src/main/kotlin/com/soundlink/soundlink/SoundLinkPlugin.kt) 写入 SharedPreferences 键 `dump_pcm`，Service 启动时读取。移动端 Flutter 侧 [`app.dart`](../../mobile/flutter_app/lib/app.dart) 在 `_init()` 时调 `platform.setDumpPcm(DUMP_ENABLE)` 同步初始状态，运行时仍可在「设备发现」页的「调试：保存采集 PCM」开关手动切换。

> **Android 抓取 dump 命令**：
> ```bash
> adb shell ls /sdcard/Download/soundlink_dump/
> adb pull /sdcard/Download/soundlink_dump/ ./dump/
> ```

### 转储文件解析

```bash
# PCM raw → WAV（任意端 PCM 文件通用）
ffmpeg -f s16le -ar 48000 -ac 2 -i soundlink_pcm_decoded.raw out.wav

# Audacity：导入 → 原始数据 → Signed 16-bit PCM / Little-endian / Stereo / 48000 Hz
```

Opus bin 文件为长度前缀帧序列，可写小脚本逐帧解析后用 `opusdec` 或 libopus 解码对照。

### 兼容入口（旧用法）

桌面接收器仍支持环境变量 `SOUNDLINK_DUMP=1` 强制开启转储（与 `DUMP_ENABLE` 任一为真即启用），便于在不重编译时临时抓 dump：

```powershell
$env:SOUNDLINK_DUMP="1"; cargo run --example phase5_loopback
```

### 安全提示

- 转储文件含原始音频，**勿提交仓库**（已加入 [`.gitignore`](../../.gitignore)）。
- 转储仅在 DEBUG 开发期使用；发布构建 `DEBUG=false` 时所有 dump 路径自动失效。

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
