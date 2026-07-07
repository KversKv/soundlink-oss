<!-- FT-0012 -->

# 音频参数与自动探测修复实录（2026-07-07）

> 场景：用户反馈桌面端调整音频参数只看到 Jitter 日志且效果不明显，手机端自动探测没有可见结果，并提供 Android 运行日志中 gralloc4 ERROR。

## 问题分析

| 问题 | 结论 |
|---|---|
| 桌面端参数不明显 | 当前全链路采集、包头、解码与输出仍以 48kHz/Stereo/10ms 为基线，之前只有 Jitter 运行时明确生效，码率未进入桌面 Sender Opus 构造路径。 |
| 手机端自动探测 | 之前仅根据当前 Jitter 做静态推荐，没有暂停音频流、没有真实探测，也没有弹窗反馈。 |
| Android 日志 ERROR | `E/gralloc4 ... unsupported format 0x3b` 伴随 Flutter Impeller/Vulkan 初始化，更像图形后端兼容性噪声，不是音频采集或自动探测异常。 |

## 实现清单

| 文件 | 变更 |
|---|---|
| `desktop/src-tauri/src/audio/opus_codec.rs` | 增加 codec 码率设置入口，桌面 Sender 可按配置初始化 Opus bitrate。 |
| `desktop/src-tauri/src/sender.rs` | 传入音频参数，发送端日志显示实际生效参数，包头保持当前基线避免与 PCM 链路不一致。 |
| `desktop/src-tauri/src/config/mod.rs` | 规范化时固定采样率、声道、帧长为 48kHz/Stereo/10ms，仅允许码率与 Jitter 变化。 |
| `desktop/ui/src/App.tsx` | UI 选项和提示改为当前真实支持边界，自动探测后展示推荐说明。 |
| `mobile/flutter_app/lib/app.dart` | 自动探测返回结果对象；探测前暂停广播；对控制端口做多次连接延迟采样并推荐参数。 |
| `mobile/flutter_app/lib/src/pages/settings_page.dart` | 自动探测后弹窗显示推荐参数、样本数、延迟与是否暂停音频流。 |
| `mobile/flutter_app/lib/src/constants.dart`、`trust_store.dart` | 手机端参数规范化固定到当前基线，避免 UI/配置和实际采集不一致。 |
| `mobile/flutter_app/android/app/src/main/kotlin/.../AudioCaptureService.kt` | Android 原生采集运行时强制使用基线 sample rate/channels/frame duration，保留 bitrate 生效。 |
| `mobile/ios/BroadcastExtension/PairingStateReader.swift`、`SampleHandler.swift` | iOS Extension 使用运行时基线配置，保留 bitrate 生效。 |
| `mobile/flutter_app/android/app/src/main/AndroidManifest.xml` | 关闭 Flutter Impeller 以规避 Vulkan/gralloc4 兼容性 ERROR 噪声。 |
| `docs/First/12-plan.md` | 回填阶段 4 音频参数任务备注，明确实际支持边界。 |

## 关键决策

- 当前版本不继续假装支持 44.1kHz、Mono、20ms 的端到端运行时变更，因为采集接口、重采样、Opus 帧尺寸、Receiver 解码/播放统计仍未完整动态化。
- 运行时真实支持范围收敛为：Opus 码率、Jitter、桌面音量。
- 手机自动探测会停止当前广播，避免 UDP 音频流影响探测结果；探测完成后需要用户手动重新开始广播。
- gralloc4 日志按 Flutter 渲染后端兼容性处理，关闭 Impeller 而不改音频链路。

## 验证结果

- `cargo fmt` 通过。
- `cargo check --features tauri_app` 通过。
- `cargo check --no-default-features` 通过。
- `desktop/ui npm run build` 通过。
- `dart format lib android/app/src/main/kotlin` 通过。
- `flutter analyze` 通过，0 issue。
- `flutter build apk --debug` 通过，已生成 `build/app/outputs/flutter-apk/app-debug.apk`。

## 已知边界

- 自动探测当前使用控制端口 TCP connect 延迟作为轻量指标，不是专门的 UDP 带宽/丢包探测。
- 若要完整支持采样率、声道、帧长动态变化，需要后续同步改造采集源接口、Opus frame size、UDP packet header、Receiver codec 重建、Jitter/latency 计算与输出链路。
