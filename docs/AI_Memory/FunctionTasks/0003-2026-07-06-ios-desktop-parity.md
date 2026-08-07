<!-- FT-0003 -->

# iOS + 桌面端音频坑点补齐验证（2026-07-06）

> 场景：用户已在 Android + 桌面路径完成音频杂音修复、桌面音量控制与 Android 自动静音方案，要求检查并补齐 iOS + 桌面路径是否具备同等功能与坑点防护。

## 一、结论

| 项目 | iOS + 桌面状态 | 说明 |
|---|---|---|
| 桌面真实 Opus | 已验证 | `tauri_app` 已依赖 `opus`；本次进一步改为使用 `libopus_sys` 官方 `OPUS_APPLICATION_AUDIO` 常量，避免手写值回归 |
| 欠流 Empty 不污染 Opus 解码器 | 已验证 | 桌面 receiver 是 Android/iOS 共用路径，`PopResult::Empty` 返回静音，不调用 PLC |
| 重采样 ratio=1.0 identity | 已验证 | 已有回归测试覆盖，`cargo test` 通过 |
| DebugDumper 非阻塞 | 已验证 | 桌面 dump 通过 mpsc + IO 线程，不在 cpal 回调直接写文件 |
| 桌面音量控制 | 已验证 | Rust `VolumeControl`、Tauri 命令、React 滑块是共享接收路径，iOS 发来的音频同样生效 |
| iOS 自动静音 | 不适用 | Android 的 `STREAM_MUSIC=0` 方案依赖 AudioPlaybackCapture 音量前采集；iOS ReplayKit 不允许应用静默修改全局媒体音量，本次补 UI 提示 |
| iOS 采集 PCM 提取 | 已补强 | 修复 interleaved Int16 输出仍用 `int16ChannelData!` 的风险，改从 `AudioBufferList.mData` 取交错数据 |
| iOS 真机端到端 | 待 macOS/Xcode | 当前环境是 Windows，无法构建 BroadcastExtension + libopus iOS target；已完成静态检查和桌面链路测试 |

## 二、实现清单

| 文件 | 改动 |
|---|---|
| [AudioProcessor.swift](file:///d:/CodeProject/TRAE_Projects/SoundLink/oss/mobile/ios/BroadcastExtension/AudioProcessor.swift) | iOS ReplayKit 归一化输出是 interleaved Int16，数据提取改为从 `audioBufferList.pointee.mBuffers.mData` 读取，避免 `int16ChannelData` 在交错格式下为空或不可靠 |
| [opus_codec.rs](file:///d:/CodeProject/TRAE_Projects/SoundLink/oss/desktop/src-tauri/src/audio/opus_codec.rs) | 桌面 libopus encoder 创建改用 `opusffi::OPUS_APPLICATION_AUDIO as c_int`，对应 FT-0001 坑 7 的防回归 |
| [platform_service.dart](file:///d:/CodeProject/TRAE_Projects/SoundLink/oss/mobile/flutter_app/lib/src/services/platform_service.dart) | 更新 dump 开关注释，明确 iOS 写 App Group，Android 写 Download/fallback |
| [discovery_page.dart](file:///d:/CodeProject/TRAE_Projects/SoundLink/oss/mobile/flutter_app/lib/src/pages/discovery_page.dart) | 调试 dump 开关文案改为跨平台描述，避免误导 iOS 用户去 Android 私有目录找文件 |
| [broadcast_guide_page.dart](file:///d:/CodeProject/TRAE_Projects/SoundLink/oss/mobile/flutter_app/lib/src/pages/broadcast_guide_page.dart) | iOS 广播引导新增本机外放/静音说明：不做 Android 式自动调系统媒体音量 |

## 三、关键判断

### 3.1 Android 自动静音不迁移到 iOS

Android 自动静音来自 FT-0002 的前提：`AudioPlaybackCapture` 取的是系统音量调节前 PCM，所以把 `STREAM_MUSIC` 调到 0 只影响手机扬声器，不影响转发。

iOS ReplayKit Broadcast Upload Extension 不提供等价的全局媒体流静音 API，应用也不应通过私有 API 或越权方式改系统音量。因此 iOS 路径采用用户提示：如果某些场景仍本机外放，由用户使用系统音量、静音或耳机控制。

### 3.2 iOS PCM 提取需要按交错格式处理

`AudioProcessor` 输出格式为：

```swift
AVAudioFormat(commonFormat: .pcmFormatInt16, sampleRate: 48000, channels: 2, interleaved: true)
```

交错 buffer 的 `int16ChannelData` 在 Swift/AVAudioPCMBuffer 中可能为空或不适合按 planar channel 指针读取。本次改为读取 `AudioBufferList.mBuffers.mData`，与 interleaved 内存布局一致，输出仍按 10ms / 1920 bytes 分帧。

### 3.3 桌面修复天然覆盖 iOS 发端

FT-0001 和 FT-0002 中多数坑点在桌面接收链路：Opus 解码、Jitter、重采样、cpal 输出、音量控制。桌面 receiver 是所有移动发端共享路径，因此 Android 修复同时覆盖 iOS 发来的 UDP 音频包。本次重点验证这些共享修复确实存在，并补齐 iOS 发端的 PCM 提取风险与用户提示。

## 四、验证结果

```powershell
cd desktop\src-tauri
cargo test --lib --no-default-features
# 45 passed; 0 failed

cargo test --lib --features opus --no-default-features
# 46 passed; 0 failed，含 libopus roundtrip

cargo clippy --features tauri_app --no-default-features
# 通过；保留 2 个既有 warning：commands/mod.rs unused_mut、Role 可 derive Default
```

```powershell
cd mobile\flutter_app
dart format lib\src\services\platform_service.dart lib\src\pages\discovery_page.dart lib\src\pages\broadcast_guide_page.dart
flutter analyze
# No issues found
```

IDE diagnostics：
- `AudioProcessor.swift`：无诊断
- `opus_codec.rs`：无诊断
- Flutter 三个修改文件：无诊断

## 五、待真机验证

当前 Windows 环境无法执行 iOS BroadcastExtension 的 Xcode 构建和真机 ReplayKit 流程。后续在 macOS 上需要验证：

1. Runner target 与 BroadcastExtension target 均能链接 libopus。
2. App Group `group.com.soundlink` 配置一致，Extension 能读取会话配置与 dump 开关。
3. iOS 播放普通非 DRM 音频，桌面端能收到稳定声音。
4. 开启 dump 后 App Group 容器内生成 `soundlink_dump/capture_pcm.raw` 与 `capture_opus.bin`。
5. 如本机仍外放，确认 UI 引导符合预期，不引入任何私有 API 自动静音逻辑。

## 六、关联文档

- [FT-0001 音频杂音调试实录](./0001-2026-07-06-audio-noise-debug.md)
- [FT-0002 桌面端音量控制 + Android 端自动静音归档](./0002-2026-07-06-volume-control-and-android-mute.md)
