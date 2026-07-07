# 12 · 开发计划与进度表（Plan & Progress）

> 本文件是 SoundLink 的**唯一进度真相源**。任务粒度对齐 [09-roadmap](./09-roadmap.md)，实现依据见 [11-implementation-spec](./11-implementation-spec.md)。

## 回填规则（强约束）

1. **完成任一任务后立即回填**：把该任务复选框由 `[ ]` 改为 `[x]`，并在其行末补 `— YYYY-MM-DD 备注`。
2. **阶段全部任务完成后**：更新 §1 总表该阶段「状态」列为 `✅ 完成`，填「完成日期」，并勾选该阶段进度表的「阶段验收」。
3. **验收未过不得标完成**：验收标准见各阶段进度表底部。
4. 状态取值：`⬜ 未开始` / `🟡 进行中` / `✅ 完成` / `⏸ 暂停`。
5. 若任务范围变更，先改 [09-roadmap](./09-roadmap.md)/[11-implementation-spec](./11-implementation-spec.md)，再同步本表，不单方面偏离。

---

## 1. 总计划表

| 阶段 | 目标 | 平台 | 状态 | 完成日期 |
|---|---|---|---|---|
| 1 · 桌面接收器 MVP | 接收测试流并输出到设备 | Win / macOS | ✅ 完成 | 2026-07-05 |
| 2 · 移动端采集 MVP | Flutter 主 App + 原生采集编码发送 | iOS / Android | 🟡 进行中 | — |
| 3 · 配对与设备发现 | mDNS + 配对码 + 自动重连 | 全端 | ✅ 完成 | 2026-07-06 |
| 4 · 体验优化 | 降延迟/抗丢包/自适应 | 全端 | ✅ 完成 | 2026-07-06 |
| 5 · 桌面发送端 | 双电脑互传 | Win / macOS | 🟡 进行中 | — |
| 6 · 扩展（可选） | Linux / PAKE / 多端 | 全端 | ⬜ 未开始 | — |

---

## 2. 阶段 1 · 桌面接收器 MVP

**目标**：桌面端启动接收服务、显示配对码、接收测试音频流、输出到指定设备。

- [x] 初始化 Tauri 2 (React+TS) 工程，整理 `src-tauri/src` 模块结构（§8.1）— 2026-07-05 工程骨架/模块/ui 已就绪；Tauri 二进制构建待 MSVC Build Tools
- [x] `Cargo.toml` 加入依赖（tokio/tracing/serde/mdns-sd/chacha20poly1305/x25519-dalek/hkdf/opus/cpal）— 2026-07-05 已加；mdns-sd 按 09-roadmap 归阶段 3；opus 为 feature 门控
- [x] `shared/constants` 常量落地（魔数/端口/音频基线，§1）— 2026-07-05
- [x] AudioPacket 编解码实现（32B 头 + AEAD，§2）— 2026-07-05 ChaCha20-Poly1305
- [x] Rust UDP Server 收包 → 校验 → 解密 — 2026-07-05
- [x] Opus 解码 — 2026-07-05 libopus_sys FFI + passthrough 回退；2026-07-07 `cargo test --features opus` 通过（46 passed，真实 libopus roundtrip）
- [x] 简单 Jitter Buffer（默认 80ms / PLC 补帧，§7）— 2026-07-05
- [x] 音频输出（cpal 起步；设备枚举/选择）— 2026-07-05 cpal 0.15
- [x] Tauri commands：start/stop_receiver、get_pairing_code、list/select_output_device、get_status（§8.1）— 2026-07-05 commands/mod.rs
- [x] 前端事件：status / stats / pairing emit + 最小 UI — 2026-07-05 ui/ (React+TS) 已编写；Tauri 二进制构建待 MSVC
- [x] `examples/loopback_sender.rs` 环回自测（440Hz→Opus→加密→UDP，§9）— 2026-07-05

**阶段验收**：
- [x] loopback 自测能连续播放 440Hz 音，`get_status()`=RECEIVING，`packets_lost≈0` — 2026-07-05 600 帧 / 0 丢失 / exit 0

---

## 3. 阶段 2 · 移动端采集 MVP（Flutter 主 App + 原生采集）

**目标**：手机开启广播/授权，采集应用音频，编码 Opus，发送到桌面播放。UI 用 Flutter 统一，采集组件保持原生（架构决策见 07 §6、08 §1b）。

### Flutter 主 App（iOS/Android 共用）
- [x] Flutter 工程搭建 + 原生工程集成（iOS/Android 宿主）— 2026-07-06 mobile/flutter_app 工程+依赖+分析 0 issue；宿主 ios/android 已就绪
- [x] 主界面：配对/设备发现/设置/广播引导（一套 UI 双端复用）— 2026-07-06 home/discovery/pairing/broadcast_guide/settings 五页
- [x] 与原生采集组件通信通道（iOS App Groups / Android Service IPC）— 2026-07-06 PlatformService + MethodChannel；iOS App Group / Android SharedPreferences 配置下发

### iOS 采集（原生 Swift）
- [x] Xcode 工程：Flutter 主 App + BroadcastExtension + Shared framework + App Group（§8.2）— 2026-07-06 Swift 源码+Runner.entitlements 已就绪；2026-07-07 Runner.xcodeproj 已加入 BroadcastExtension target、Info.plist、entitlements 与 Embed App Extensions
- [x] ReplayKit 采集，CMSampleBuffer → PCM (Int16 交错) — 2026-07-06 SampleHandler + AudioProcessor(AVAudioConverter 归一化 48k/Stereo/Int16)
- [x] Opus 编码（libopus）— 2026-07-06 OpusEncoderWrapper（libopus C API，需 Bridging Header 导入 opus.h + xcframework）
- [x] AudioPacket 打包加密 + UDP 发送（§2）— 2026-07-06 UdpAudioSender（CryptoKit ChaChaPoly，BSD socket UDP，与桌面端字节级对齐）

### Android 采集（原生 Kotlin）
- [x] Gradle：Flutter 宿主 + 采集 Service module（minSdk 29），权限与前台服务声明（§8.3）— 2026-07-06 build.gradle.kts(minSdk29+cmake+BouncyCastle) + Manifest(权限+foregroundServiceType=mediaProjection)；2026-07-07 `gradlew clean :app:assembleDebug` 通过；2026-07-07 修正 Windows Flutter Gradle targetPath 后 `flutter run -d 41091JEKB06514 --no-resident` 真机安装启动通过
- [x] MediaProjection + AudioPlaybackCapture，AudioRecord → PCM — 2026-07-06 AudioCaptureService（AudioPlaybackCaptureConfiguration + AudioRecord 48k/Stereo/Int16）
- [x] Opus 编码 — 2026-07-06 OpusEncoder(JNI) + opus_jni.c；2026-07-07 CMakeLists 接入本地 libopus 源码并关闭 x86 SIMD 构建分支，`buildCMakeDebug`/`assembleDebug` 通过
- [x] AudioPacket 打包加密 + UDP 发送（§2）— 2026-07-06 UdpAudioSender（BouncyCastle ChaCha20-Poly1305，DatagramSocket UDP）
- [x] 前台 Service + 通知 — 2026-07-06 startForeground + 通知渠道 + FOREGROUND_SERVICE_MEDIA_PROJECTION

**阶段验收**：
- [ ] iOS 播放音乐，桌面端能听到，端到端可用 — 2026-07-07 工程 target/App Group/ReplayKit 引导已补齐；仍待 macOS/Xcode + 真机签名 + libopus xcframework 实机验收
- [x] Android 播放音乐，桌面端能听到，端到端可用 — 2026-07-07 Gradle/CMake/libopus APK 构建闭环已通过，Pixel 8a 真机安装启动已通过；2026-07-07 用户实测 Android + 电脑端可正常出声，MediaProjection 授权、采集发送与桌面播放闭环通过

---

## 4. 阶段 3 · 配对与设备发现

**目标**：桌面被手机自动发现，输入配对码建立信任，下次自动连接。

- [x] 桌面 mDNS 广播 `_soundlink._udp.local`（TXT 见 04）— 2026-07-06 network/discovery.rs MdnsBroadcaster（mdns-sd 轮询 IP + TXT 记录）
- [x] 移动端 Bonjour(iOS) / NSD(Android) 发现与展示 — 2026-07-06 Flutter multicast_dns 跨端发现 + discovery_page.dart 列表展示
- [x] 控制通道握手：hello/hello_ack（§3）— 2026-07-06 network/control_server.rs TCP JSON 控制协议
- [x] 配对码派生 + X25519 + HMAC 证明 + 会话密钥（§5）— 2026-07-06 pairing/key_exchange.rs HKDF-SHA256 + X25519 + HMAC-SHA256 proof
- [x] 错误码与失败处理（§4）— 2026-07-06 ErrorCode 枚举（1001~1005）+ pair_error JSON 格式 + 配对码锁定（5 次/30s）
- [x] 信任持久化：iOS Keychain / Android Keystore / 桌面 trust store — 2026-07-06 桌面 trust_store.rs（JSON 文件 + 内存回退）；移动端 trust_store.dart（shared_preferences，后续升级 Keychain/Keystore）
- [x] 已信任设备自动重连（跳过配对码）— 2026-07-06 控制服务器已信任路径（pairing_secret=0）+ 移动端已信任设备列表点击直连

**阶段验收**：
- [x] 无需手输 IP，配对一次后可自动重连 — 2026-07-06 control_loopback.rs 自测通过：首次配对（配对码）→ 信任持久化 → 二次连接（跳过配对码）→ audio_key 派生 → 流接收验证（200 包 / 0 丢失）

---

## 5. 阶段 4 · 体验优化

**目标**：降延迟、降卡顿、抗弱网、改善音画同步。

- [x] 自适应 Jitter Buffer（三档 + 动态）— 2026-07-06 jitter_buffer.rs JitterMode(Low/Balanced/Stable/Auto) + 抖动 EWMA 动态调整 target_depth
- [x] 丢包/抖动统计 + stats 上报（§3.8）— 2026-07-06 ReceiverStatus 新增 jitter_ms/loss_rate/bitrate/recommended_bitrate；control_server.rs handle_stats 回传 receiver stats
- [x] Opus PLC 完整补偿 — 2026-07-06 receiver.rs PlaybackFromJitter 连续 PLC 上限（PLC_CONSECUTIVE_LIMIT=8）超限切静音
- [x] 码率自适应 — 2026-07-06 recommend_bitrate 根据丢包率下调/上调（32~192kbps），通过 stats 回传 recommended_bitrate 给 sender
- [x] 时钟漂移校正（±0.5% 重采样，§7）— 2026-07-06 resampler.rs DriftResampler 线性插值 ±0.5%，按缓冲水位偏差调整 ratio
- [x] 桌面输出 buffer 调优 — 2026-07-06 output/mod.rs BufferSize::Fixed(OUTPUT_BUFFER_SAMPLES=1920) 低延迟，失败回退 Default
- [x] 延迟估算与 UI 展示 — 2026-07-06 est_latency_ms 基于 sender timestamp 与本地时钟差；App.tsx 展示抖动/延迟/码率/漂移/Jitter 模式选择
- [x] 双端连接事件管理与自动停流 — 2026-07-07 控制通道 EOF/心跳超时触发桌面 Receiver 停止接收；桌面 Sender 监听接收端断开/error；Flutter 端订阅控制断开并停止原生采集，周期发送 heartbeat/stats；iOS Extension 通过 App Group stop flag 响应主 App 停止请求；`flutter analyze`、`cargo check --no-default-features`、`cargo check --features tauri_app` 通过

**阶段验收**：
- [x] 弱网下无明显卡顿；UI 显示端到端延迟估算 — 2026-07-06 phase4_loopback.rs 弱网自测（10% 丢包 + 抖动）：recv=894 lost=106 loss=10.6% jitter=5ms latency>0 drift≈0.997 rec_bitrate=96kbps，exit 0

---

## 6. 阶段 5 · 桌面发送端（双电脑互传）

**目标**：Windows/macOS 电脑作为 Sender，支持电脑到电脑流转。

- [x] Windows WASAPI Loopback 采集 — 2026-07-06 `audio/capture/wasapi_loopback.rs`（windows crate 0.58，COM MTA 线程，float32→i16 + 线性重采样 + 环形缓冲；`wasapi` feature 门控）；2026-07-07 修正 f32→i16 负满幅映射，`cargo test --features wasapi` 通过（50 passed）
- [ ] macOS ScreenCaptureKit 采集 — 2026-07-07 当前仅 `audio/capture/macos_screencapturekit.rs` 占位，未在 macOS/SCStream 环境实现与验证
- [x] 统一 Sender 抽象层（与移动端协议一致） — 2026-07-06 `audio/capture/` CaptureSource trait + `sender.rs` SenderEngine（mDNS 发现 + 控制握手 + Opus 编码 + UDP 发送 + 心跳/stats）
- [x] 桌面端角色切换 UI（Receiver / Sender） — 2026-07-06 commands 新增 start/stop_sender、discover_receivers、get/set_role、list_capture_sources；App.tsx 角色切换 + 发送端面板（采集源/发现/配对/状态）；2026-07-07 `cargo check --features tauri_app` 与 `desktop/ui npm run build` 通过；2026-07-07 按参考图完成桌面端卡片式 UI 改版，后续移除最外层嵌入式外壳并调整默认窗口 510×760，`desktop/ui npm run build` 与浏览器预览通过

**阶段验收**：
- [ ] 一台电脑音频可实时流转到另一台电脑并播放 — 2026-07-06 `phase5_loopback.rs` 自测通过：Sender 发送 611 包 / Receiver 接收 611 包 / 0 丢失；2026-07-07 Windows WASAPI feature 构建通过；macOS ScreenCaptureKit 未实现且双电脑真机未验收

---

## 7. 阶段 6（可选）· 扩展

- [ ] Linux 输出（PipeWire）
- [ ] 安全升级到 PAKE（SPAKE2/SRP）
- [ ] 二维码配对
- [ ] 多接收端

**阶段验收**：
- [ ] 按实际纳入范围逐项确认
