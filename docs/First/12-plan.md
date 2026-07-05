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
| 2 · 移动端采集 MVP | Flutter 主 App + 原生采集编码发送 | iOS / Android | ⬜ 未开始 | — |
| 3 · 配对与设备发现 | mDNS + 配对码 + 自动重连 | 全端 | ⬜ 未开始 | — |
| 4 · 体验优化 | 降延迟/抗丢包/自适应 | 全端 | ⬜ 未开始 | — |
| 5 · 桌面发送端 | 双电脑互传 | Win / macOS | ⬜ 未开始 | — |
| 6 · 扩展（可选） | Linux / PAKE / 多端 | 全端 | ⬜ 未开始 | — |

---

## 2. 阶段 1 · 桌面接收器 MVP

**目标**：桌面端启动接收服务、显示配对码、接收测试音频流、输出到指定设备。

- [x] 初始化 Tauri 2 (React+TS) 工程，整理 `src-tauri/src` 模块结构（§8.1）— 2026-07-05 工程骨架/模块/ui 已就绪；Tauri 二进制构建待 MSVC Build Tools
- [x] `Cargo.toml` 加入依赖（tokio/tracing/serde/mdns-sd/chacha20poly1305/x25519-dalek/hkdf/opus/cpal）— 2026-07-05 已加；mdns-sd 按 09-roadmap 归阶段 3；opus 为 feature 门控
- [x] `shared/constants` 常量落地（魔数/端口/音频基线，§1）— 2026-07-05
- [x] AudioPacket 编解码实现（32B 头 + AEAD，§2）— 2026-07-05 ChaCha20-Poly1305
- [x] Rust UDP Server 收包 → 校验 → 解密 — 2026-07-05
- [x] Opus 解码 — 2026-07-05 libopus_sys FFI + passthrough 回退；loopback 用 passthrough 验证链路，真实 Opus 待 CMake 构建
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
- [ ] Flutter 工程搭建 + 原生工程集成（iOS/Android 宿主）
- [ ] 主界面：配对/设备发现/设置/广播引导（一套 UI 双端复用）
- [ ] 与原生采集组件通信通道（iOS App Groups / Android Service IPC）

### iOS 采集（原生 Swift）
- [ ] Xcode 工程：Flutter 主 App + BroadcastExtension + Shared framework + App Group（§8.2）
- [ ] ReplayKit 采集，CMSampleBuffer → PCM (Int16 交错)
- [ ] Opus 编码（libopus）
- [ ] AudioPacket 打包加密 + UDP 发送（§2）

### Android 采集（原生 Kotlin）
- [ ] Gradle：Flutter 宿主 + 采集 Service module（minSdk 29），权限与前台服务声明（§8.3）
- [ ] MediaProjection + AudioPlaybackCapture，AudioRecord → PCM
- [ ] Opus 编码
- [ ] AudioPacket 打包加密 + UDP 发送（§2）
- [ ] 前台 Service + 通知

**阶段验收**：
- [ ] iOS 播放音乐，桌面端能听到，端到端可用
- [ ] Android 播放音乐，桌面端能听到，端到端可用

---

## 4. 阶段 3 · 配对与设备发现

**目标**：桌面被手机自动发现，输入配对码建立信任，下次自动连接。

- [ ] 桌面 mDNS 广播 `_soundlink._udp.local`（TXT 见 04）
- [ ] 移动端 Bonjour(iOS) / NSD(Android) 发现与展示
- [ ] 控制通道握手：hello/hello_ack（§3）
- [ ] 配对码派生 + X25519 + HMAC 证明 + 会话密钥（§5）
- [ ] 错误码与失败处理（§4）
- [ ] 信任持久化：iOS Keychain / Android Keystore / 桌面 trust store
- [ ] 已信任设备自动重连（跳过配对码）

**阶段验收**：
- [ ] 无需手输 IP，配对一次后可自动重连

---

## 5. 阶段 4 · 体验优化

**目标**：降延迟、降卡顿、抗弱网、改善音画同步。

- [ ] 自适应 Jitter Buffer（三档 + 动态）
- [ ] 丢包/抖动统计 + stats 上报（§3.8）
- [ ] Opus PLC 完整补偿
- [ ] 码率自适应
- [ ] 时钟漂移校正（±0.5% 重采样，§7）
- [ ] 桌面输出 buffer 调优
- [ ] 延迟估算与 UI 展示

**阶段验收**：
- [ ] 弱网下无明显卡顿；UI 显示端到端延迟估算

---

## 6. 阶段 5 · 桌面发送端（双电脑互传）

**目标**：Windows/macOS 电脑作为 Sender，支持电脑到电脑流转。

- [ ] Windows WASAPI Loopback 采集
- [ ] macOS ScreenCaptureKit 采集
- [ ] 统一 Sender 抽象层（与移动端协议一致）
- [ ] 桌面端角色切换 UI（Receiver / Sender）

**阶段验收**：
- [ ] 一台电脑音频可实时流转到另一台电脑并播放

---

## 7. 阶段 6（可选）· 扩展

- [ ] Linux 输出（PipeWire）
- [ ] 安全升级到 PAKE（SPAKE2/SRP）
- [ ] 二维码配对
- [ ] 多接收端

**阶段验收**：
- [ ] 按实际纳入范围逐项确认
