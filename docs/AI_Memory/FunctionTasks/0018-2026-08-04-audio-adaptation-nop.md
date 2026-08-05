<!-- FT-0018 -->

# 音频自适应 N/O/P 三阶段实装实录（2026-08-04）

> 场景：依据 [`docs/NewFunctions/audio-adaptation/00-audio-adaptation-plan.md`](../../NewFunctions/audio-adaptation/00-audio-adaptation-plan.md) 完成音频参数体系三阶段开发——N 码率自适应闭环、O 真实探测、P 参数动态化。

## 背景

审计发现「协议与 UI 完整、运行时生效面窄」：`recommended_bitrate` 回传后不进编码器、`probe_request` 只回 `accepted`、采样率/声道/帧长被强制归一化为 48k/Stereo/10ms。本文记录三阶段的实装与关键决策。

## 实现清单

### 阶段 N · 码率自适应闭环

| 文件 | 改动 |
|---|---|
| [sender.rs](../../../desktop/src-tauri/src/sender.rs) | `target_bitrate: Arc<AtomicU32>` + `bitrate_adaptive: Arc<AtomicBool>`；`send_loop` 循环内检测目标码率变化 → `codec.set_bitrate()` 热下发；`handle_control_message` STATS 分支在 auto 时把建议值归档后写入 target |
| [commands/mod.rs](../../../desktop/src-tauri/src/commands/mod.rs) | `set_audio_params`：auto 开自适应、手动下发目标码率 |
| [AudioCaptureService.kt](../../../mobile/flutter_app/android/app/src/main/kotlin/com/soundlink/soundlink/capture/AudioCaptureService.kt) | `ACTION_SET_BITRATE` + SharedPreferences pending 轮询热下发 |
| [OpusEncoder.kt](../../../mobile/flutter_app/android/app/src/main/kotlin/com/soundlink/soundlink/codec/OpusEncoder.kt) + [opus_jni.c](../../../mobile/flutter_app/android/app/src/main/cpp/opus_jni.c) | `nativeSetBitrate` JNI |
| [SampleHandler.swift](../../../mobile/ios/BroadcastExtension/SampleHandler.swift) + [OpusEncoderWrapper.swift](../../../mobile/ios/BroadcastExtension/OpusEncoderWrapper.swift) | App Group `pending_bitrate` 轮询 + `setBitrate` |
| [pairing_service.dart](../../../mobile/flutter_app/lib/src/services/pairing_service.dart) | `_onReceiverStats` 归档节流下发 |
| [App.tsx](../../../desktop/ui/src/App.tsx) | 建议码率展示 + 一键采纳 + 自适应标注 |

### 阶段 O · 真实探测

| 文件 | 改动 |
|---|---|
| [commands/mod.rs](../../../desktop/src-tauri/src/commands/mod.rs) | `auto_detect_audio_params` 样本不足（`packets_recv < PROBE_MIN_PACKETS`）保持当前参数 |
| [control_server.rs](../../../desktop/src-tauri/src/network/control_server.rs) | `handle_probe_request` 基于真实统计回传 `probe_result` |
| [receiver.rs](../../../desktop/src-tauri/src/receiver.rs) | `FLAG_PROBE` 探测包回显分流（不进 Jitter/统计） |
| [app.dart](../../../mobile/flutter_app/lib/app.dart) + [pairing_service.dart](../../../mobile/flutter_app/lib/src/services/pairing_service.dart) | 移动端改走 `probe_request`/`probe_result`，不再强制停流；阈值统一 `loss_rate`/`jitter_ms` 口径 |

### 阶段 P · 参数动态化

| 文件 | 改动 |
|---|---|
| [constants.rs](../../../desktop/src-tauri/src/constants.rs) | 运行时 `AudioFormat` 结构（白名单/派生样本数/`is_baseline`） |
| [opus_codec.rs](../../../desktop/src-tauri/src/audio/opus_codec.rs) | `LibopusCodec::with_format` + `codec_with_format` + trait `format()` |
| [format_convert.rs](../../../desktop/src-tauri/src/audio/format_convert.rs) | 新增：线性插值重采样 + 声道映射（发送端编码前 / 接收端解码后转换） |
| [sender.rs](../../../desktop/src-tauri/src/sender.rs) | 采集基线帧 → `to_session` → 凑满会话帧长编码，包头带会话格式 |
| [receiver.rs](../../../desktop/src-tauri/src/receiver.rs) | `start_with_format` 重建解码器；`PlaybackFromJitter` 解码 → `to_baseline` → 漂移校正 |
| [control_server.rs](../../../desktop/src-tauri/src/network/control_server.rs) | `handle_stream_start` 解析会话格式；`restart_required` 真实判定 |
| Android/iOS/Flutter | `SessionFormatConverter.kt` / `SessionFormatConverter.swift` / 白名单常量 |

## 关键设计决策

1. **务实转换架构**：采集与播放始终工作于 48k/Stereo 设备基线，会话格式差异集中在「编码前」与「解码后」两点的轻量转换（线性插值 + 声道映射）。WASAPI、AudioProcessor、cpal 输出、漂移校正全部零改动，回归风险最小。
2. **帧长跨拍凑帧**：采集节拍固定 10ms，20ms 会话帧经累积缓冲凑满后再编码，保证 Opus 帧边界正确。
3. **码率自适应节流**：最短间隔 5s + 建议值归档到 UI 允许集合，避免音质忽高忽低。

## ⚠ 重要发现：采样率收窄

spec §3.9 原声明 `sample_rate=44100|48000`，但 **libopus 仅支持 8/12/16/24/48kHz**——44100 导致 `opus_encoder_create` 返回 `OPUS_BAD_ARG(-1)`（端到端自测实测复现，编码器回退 passthrough 暴露）。故会话采样率收窄为固定 48kHz，动态化维度保留声道（Mono/Stereo）与帧长（10/20ms）。已同步 `11-implementation-spec.md` §3.9 与双端白名单，消除文档-实现不一致。

## 验证结果

- `cargo test --lib`：70 passed / 0 failed。
- `cargo clippy --lib`：无警告。
- `npm run build`（desktop/ui）：通过。
- `flutter analyze`：No issues found。
- 端到端自测 `examples/phase_p_format.rs --features opus`：48k/Mono/20ms 真实 Opus 收发 305 包零丢失、码率 ~115kbps（96k VBR 实测）。
- 基线回归 `examples/phase5_loopback.rs --features opus`：609 包零丢失，未破坏现有链路。

## 用户需自行完成

- Android/iOS 真机验证：N3 码率热下发（adb logcat 观察 encoder 码率变化）、O4 探测不中断广播、P 阶段 Mono/20ms 实机收发。
- 弱网注入复测：N 阶段码率自动下调/回升（`examples/phase4_loopback.rs` 注入丢包）。
- Kotlin/Swift 侧无法本地编译验证，需 IDE/真机构建确认。

## 已知边界

- 主动 UDP 探测序列（`PROBE_PACKET_COUNT`/`PROBE_INTERVAL_MS` 常量已预留）未实现——探测结论由 O3 控制面基于真实 UDP 统计回传满足需求；如需开流前预估带宽可后续补。
- `flags` 字段文档由 u16 更正为 u8（与实现一致），属文档修正非协议变更。

## 建议版本级别

**MINOR（0.x 阶段走 MINOR）**。理由：新增用户可感知能力（码率自适应、真实探测、Mono/20ms 动态化）；协议字段语义收窄（采样率）但 48k 基线行为不变、双端同步收窄，不构成运行时破坏性变更；`flags` 文档修正无协议影响。无需用户迁移动作。

## 关联文档

- 计划与回填：[`docs/NewFunctions/audio-adaptation/00-audio-adaptation-plan.md`](../../NewFunctions/audio-adaptation/00-audio-adaptation-plan.md)
- 历史决策：[FT-0012](./0012-2026-07-07-audio-params-probe-fix.md)（当时收敛支持面到码率+Jitter+音量，本次重新扩展）
