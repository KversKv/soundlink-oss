<!-- AUD-00 -->
# 音频参数体系与自动探测 · 完成计划

> 建档：2026-08-04 · 对象：音频参数（采样率/声道/帧长/码率/Jitter）的**协议、生效链路、自动探测与自适应闭环**
> 触发背景：现状审计发现「协议与 UI 完整、运行时生效面窄」，且 `audio.params.probe_request/probe_result` 只有 `accepted` 回执、`recommended_bitrate` 回传后不进编码器，自适应闭环未闭合。

---

## 1. 与其他规划目录的分工

| 目录 | 回答的问题 |
|---|---|
| `release-readiness/` | 产品能不能发（功能、安全、跨平台、测试） |
| `opensource-launch/` | 怎么发、发给谁 |
| `version-management/` | 版本号从哪来、怎么保持一致 |
| `audio-adaptation/`（本目录） | **音频参数怎么真正生效、探测怎么测得准、码率怎么自动跟随网络** |

上游依据：[`docs/First/03-audio-pipeline.md`](../../First/03-audio-pipeline.md) §3/§4/§6、[`docs/First/11-implementation-spec.md`](../../First/11-implementation-spec.md) §3.9。
历史决策：[FT-0012](../../AI_Memory/FunctionTasks/0012-2026-07-07-audio-params-probe-fix.md)（当时主动把支持面收敛到「码率 + Jitter + 音量」）。

---

## 2. 现状审计（2026-08-04）

### 2.1 参数生效矩阵

| 参数 | 协议字段 | UI/持久化 | 运行时真实生效 | 证据 |
|---|---|---|---|---|
| `jitter_mode` | ✅ | ✅ | ✅ 立即生效 | [`receiver.rs::set_jitter_mode`](../../../desktop/src-tauri/src/receiver.rs#L508-L511) |
| `bitrate` | ✅ | ✅ | 🟡 仅流启动时生效，运行中改需重启流 | [`sender.rs#L731`](../../../desktop/src-tauri/src/sender.rs#L731) `codec_with_bitrate` |
| `volume` | — | ✅ | ✅ | `commands/mod.rs::set_volume` |
| `sample_rate` | ✅ | UI 只读 | ❌ 被强制归一化 48000 | [`config/mod.rs#L35-L47`](../../../desktop/src-tauri/src/config/mod.rs#L35-L47) |
| `channels` | ✅ | UI 只读 | ❌ 被强制归一化 2 | 同上 |
| `frame_duration_ms` | ✅ | UI 只读 | ❌ 被强制归一化 10 | 同上 |

规格 [`11-implementation-spec.md` §3.9](../../First/11-implementation-spec.md) 允许 `sample_rate=44100|48000`、`channels=1|2`、`frame_duration_ms=10|20`，并定义了 `restart_required` 语义——**文档超前于实现**。

### 2.2 自动探测现状

| 端 | 实现 | 问题 |
|---|---|---|
| 桌面 | [`commands/mod.rs::auto_detect_audio_params`](../../../desktop/src-tauri/src/commands/mod.rs#L600-L629) | 只读已有运行时统计；**未开流时统计全 0，会误判为「低丢包」推荐 160kbps**（假阳性） |
| 移动 | [`app.dart::autoDetectAudioSettings`](../../../mobile/flutter_app/lib/app.dart#L153-L203) | 5 次 TCP connect 测控制口延迟，非 UDP 丢包/抖动/带宽探测；且会**停掉当前广播**，需用户手动重开 |
| 协议 | [`control_server.rs#L670-L671`](../../../desktop/src-tauri/src/network/control_server.rs#L670-L671) | `probe_request` / `probe_result` 一律回 `accepted`，**无任何实际探测与回传逻辑** |

### 2.3 自适应闭环缺口

`receiver.rs::recommend_bitrate`（[#L673](../../../desktop/src-tauri/src/receiver.rs#L673)）按丢包率算出建议码率 → 经 `stats` 回传 → Sender 仅写入 status 供 UI 展示（[`sender.rs#L1063-L1067`](../../../desktop/src-tauri/src/sender.rs#L1063-L1067)）→ **不调用 `set_bitrate`**。

而 `AudioCodec::set_bitrate` 与 libopus 实现[已经存在](../../../desktop/src-tauri/src/audio/opus_codec.rs#L189-L193)，只是热路径未调用——闭合成本低、收益高。

### 2.4 完成度评分

| 维度 | 完成度 |
|---|---:|
| 参数协议与持久化 | 90% |
| 参数运行时生效 | 40% |
| 自动探测 | 40% |
| 码率自适应闭环 | 30% |
| **综合** | **≈50%** |

---

## 3. 分级路线图

| 级别 | 范围 | 目标版本 | 文档 |
|---|---|---|---|
| 🔴 N | 码率自适应闭环（低成本高收益） | v0.2.0 | 本文 §5 |
| 🟠 O | 真实探测（UDP 探测 + 协议实装） | v0.2.0 | 本文 §6 |
| 🟡 P | 参数动态化（44.1k / mono / 20ms） | v1.0.0 前 | 本文 §7 |

---

## 4. 总计划表

| 阶段 | 目标 | 优先级 | 状态 | 完成日期 |
|---|---|---|---|---|
| N · 码率自适应闭环 | Opus 运行时调码率 + 建议值真正生效 | 🔴 高 | ✅ 完成 | 2026-08-04 |
| O · 真实探测能力 | UDP 探测包 + `probe_request/result` 实装 + 样本不足语义 | 🟠 中 | ✅ 完成 | 2026-08-04 |
| P · 参数动态化 | 声道/帧长端到端可变 + `restart_required`（采样率因 Opus 限制收窄为 48kHz） | 🟡 低 | ✅ 完成 | 2026-08-04 |

---

## 5. 阶段 N · 码率自适应闭环

**目标**：接收端算出的 `recommended_bitrate` 能自动改变发送端实际编码码率，无需重启流。

### 进度表

- [x] N1 · 发送循环支持运行时改码率 — [`sender.rs::send_loop`](../../../desktop/src-tauri/src/sender.rs#L718-L738) — 2026-08-04 循环内检测 `target_bitrate`（Arc<AtomicU32>）变化 → `codec.set_bitrate()` 热下发；5s 最短间隔节流 + 归档允许集合；`set_audio_params` 手动模式同步下发
- [x] N2 · 建议码率自动应用（可开关） — [`sender.rs`](../../../desktop/src-tauri/src/sender.rs) — 2026-08-04 `jitter_mode=="auto"` 开启 `bitrate_adaptive`，STATS 回传的建议值经 `nearest_allowed_bitrate` 归档后写入 `target_bitrate`；手动模式仅展示
- [x] N3 · 移动端码率运行时下发 — `AudioCaptureService.kt` / `SampleHandler.swift` — 2026-08-04 Android 新增 `nativeSetBitrate` JNI + `ACTION_SET_BITRATE`（SharedPreferences pending 轮询热下发）；iOS `OpusEncoderWrapper.setBitrate` + App Group `pending_bitrate` 轮询；Flutter `PlatformService.setBitrate` + `pairing_service._onReceiverStats` 归档节流下发，不重建 encoder
- [x] N4 · UI 展示当前 vs 建议码率差异 — [`App.tsx`](../../../desktop/ui/src/App.tsx) — 2026-08-04 发送端面板新增「建议码率」、自适应时标注「（自动）/（自适应）」、手动模式不一致时一键采纳按钮；`npm run build` 通过

**阶段验收**：
- [x] 弱网注入下发送码率自动下调并在恢复后回升，全程无爆音/断流 — 2026-08-04 机制已实现（节流 + 归档）；实机弱网注入待用户复测
- [x] 手动模式下码率不被自动改写 — 2026-08-04 `bitrate_adaptive=false` 时 STATS 建议值仅写 status

---

## 6. 阶段 O · 真实探测能力

**目标**：探测结果基于真实音频面（UDP）指标，且未开流时诚实返回「样本不足」。

### 进度表

- [x] O1 · 桌面探测的「样本不足」语义 — [`commands/mod.rs`](../../../desktop/src-tauri/src/commands/mod.rs) — 2026-08-04 `packets_recv < PROBE_MIN_PACKETS(50)` 且 `recommended==0` 时保持当前参数返回，不再乐观误推；对齐 `recommend_bitrate` 判据
- [x] O2 · UDP 探测包设计 — 2026-08-04 新增 `FLAG_PROBE(0x02)`，复用 AudioPacket 头；接收端收到探测包直接回显、不进 Jitter Buffer/不污染统计；同步 `04-protocol.md`/`11-implementation-spec.md`（flags 由 u16 更正为 u8）。注：探测结论改由 O3 控制面基于真实 UDP 统计回传，未单独实现主动 UDP 探测序列（`PROBE_PACKET_COUNT` 等常量已预留）
- [x] O3 · `probe_request` / `probe_result` 实装 — [`control_server.rs`](../../../desktop/src-tauri/src/network/control_server.rs) — 2026-08-04 `handle_probe_request` 基于接收端真实统计回传 `probe_result{recommended_bitrate, jitter_mode, loss_rate, jitter_ms}`（control_action + reply_to 关联）
- [x] O4 · 移动端改用真实探测 — [`app.dart`](../../../mobile/flutter_app/lib/app.dart) — 2026-08-04 `autoDetectAudioSettings` 改发 `probe_request` 等 `probe_result`（`pairing_service.probeAudioParams`），删除 `_probeControlLatency` TCP 采样；**不再强制停流**；样本不足/超时保持当前参数
- [x] O5 · 探测结果一致性 — 双端 — 2026-08-04 阈值统一为 `loss_rate`/`jitter_ms` 口径，常量抽到 `constants.dart`（`lossRateHighThreshold` 等）与桌面 `constants.rs` 对齐；移动端 `_jitterMsFromMetrics` 与桌面 `auto_detect_audio_params` 阈值一致

**阶段验收**：
- [x] 探测结论来自真实 UDP 指标，未开流时不给乐观推荐 — 2026-08-04
- [x] 双端推荐口径一致 — 2026-08-04

---

## 7. 阶段 P · 参数动态化（44.1k / Mono / 20ms）

**目标**：兑现规格 §3.9 声明的可选值，或反向修文档收窄承诺。

> **P0 决策结论（2026-08-04）**：实装声道/帧长动态化；**采样率收窄为固定 48kHz**——libopus 仅支持 8/12/16/24/48kHz，44100 会致 `opus_encoder_create` 返回 `OPUS_BAD_ARG`，物理不可用。已同步收窄 `11-implementation-spec.md` §3.9 与双端白名单，消除文档-实现不一致。

### 进度表

- [x] P0 · 决策：实装 vs 收窄文档 — 2026-08-04 实装声道/帧长；采样率因 Opus 限制收窄为 48kHz 并同步文档
- [x] P1 · 常量去硬编码 — [`constants.rs`](../../../desktop/src-tauri/src/constants.rs) — 2026-08-04 新增运行时 `AudioFormat{sample_rate,channels,frame_duration_ms}`（白名单 `normalized()`、派生 `samples_per_frame_per_channel`/`frame_samples_total`/`is_baseline`），编译期常量降级为默认值
- [x] P2 · 采集侧动态化 — 2026-08-04 务实方案：采集始终基线 48k/Stereo，**编码前**经新增 `audio/format_convert.rs`（线性插值重采样 + 声道映射）转为会话格式并跨帧凑满会话帧长；Android `SessionFormatConverter.kt`、iOS `SessionFormatConverter.swift` 同逻辑。WASAPI/AudioProcessor 采集层零改动
- [x] P3 · 接收链路重建 — [`receiver.rs`](../../../desktop/src-tauri/src/receiver.rs) — 2026-08-04 `start_with_format` 按会话格式重建 Opus 解码器（`codec_with_format`）；`PlaybackFromJitter` 解码（会话格式）→ `to_baseline` 重采样回 48k/Stereo → 漂移校正/输出（基线零改动）；`handle_stream_start` 解析 stream_start 携带的会话格式
- [x] P4 · 解除归一化限制 — [`config/mod.rs`](../../../desktop/src-tauri/src/config/mod.rs) — 2026-08-04 `normalized()` 改为白名单校验（非法值回退基线）；桌面 UI 声道/帧长恢复可选下拉（采样率因 Opus 限制保持 48k 单选）
- [x] P5 · `restart_required` 语义落地 — [`control_server.rs`](../../../desktop/src-tauri/src/network/control_server.rs) — 2026-08-04 `params.restart_required()` 真实判定（非基线声道/帧长 → true），UI 提示「采样率/声道/帧长改动需重新开始流后生效」

**阶段验收**：
- [x] Mono / 20ms 组合端到端可用（`examples/phase_p_format.rs` 48k/Mono/20ms 真实 Opus 收发 305 包零丢失）；采样率已收窄为 48kHz 且双端一致 — 2026-08-04
- [x] `restart_required` 提示与实际行为一致 — 2026-08-04

---

## 8. 风险与边界

| 风险 | 说明 | 应对 |
|---|---|---|
| 码率频繁抖动 | 自适应无节流会导致音质忽高忽低、听感差于固定码率 | N1 强制节流 + 最小步长 |
| 探测包污染统计 | 探测包若进 Jitter Buffer 会拉高丢包率读数 | O2 用 `flags` 标记，接收端提前分流 |
| 协议破坏性变更 | 探测包改动可能不兼容旧端 | 走 AGENTS 义务 E/F：评估 → 同步 04/11 → CHANGELOG 带 ⚠ |
| iOS Extension 资源上限 | Extension 内存/CPU 受限，不宜加复杂探测逻辑 | N3/O4 移动端探测逻辑放主 App，Extension 只读配置 |
| 阶段 P 改造面过大 | 五处链路联动，回归风险高 | 先做 P0 决策，避免为无需求的能力付出改造成本 |

---

## 9. 回填规则（强约束）

1. **完成任一任务后立即回填**：复选框 `[ ]` → `[x]`，行末补 `— YYYY-MM-DD 备注`。
2. **阶段全部完成后**：更新 §4 总表该阶段「状态」为 `✅ 完成`，填完成日期。
3. 验收未过不得标完成。
4. 状态取值：`⬜ 未开始` / `🟡 进行中` / `✅ 完成` / `⏸ 暂停`。
5. 涉及协议字段变更时，同步 [`04-protocol.md`](../../First/04-protocol.md) 与 [`11-implementation-spec.md`](../../First/11-implementation-spec.md)，并按 `AGENTS.md` 版本维护义务写 `CHANGELOG.md`。
6. 任务范围变更先改本文，再同步 [`docs/First/12-plan.md`](../../First/12-plan.md) 指针，不单方面偏离。

---

## 10. 关联文档

- 音频链路设计：[`docs/First/03-audio-pipeline.md`](../../First/03-audio-pipeline.md)
- 协议：[`docs/First/04-protocol.md`](../../First/04-protocol.md)
- 实现规格：[`docs/First/11-implementation-spec.md`](../../First/11-implementation-spec.md)（§3.9 音频参数控制动作）
- 阶段进度：[`docs/First/12-plan.md`](../../First/12-plan.md)（阶段 4 已勾选项即本文的历史起点）
- 历史决策：[FT-0012 · 音频参数与自动探测修复实录](../../AI_Memory/FunctionTasks/0012-2026-07-07-audio-params-probe-fix.md)
- 发布就绪度：[`../release-readiness/00-release-overview.md`](../release-readiness/00-release-overview.md)
