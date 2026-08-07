<!-- OSL-01 -->
# 市场调研与差异化定位

> 调研日期：2026-08-03 · 方式：公开资料检索（官网 / GitHub / 应用商店说明）
> 用途：为 GitHub 首发确定定位与文案卖点，识别功能缺口的优先级。
> 说明：延迟数字均为**各方自我宣称或社区反馈**，非本项目统一环境实测，仅作量级参考。

---

## 1. 需求场景界定

核心场景：**手机播放的音频，想用电脑连接的耳机 / 声卡 / 音箱来听**。

派生场景：电脑 → 电脑（把 A 机声音送到 B 机的音频设备）。

用户在意的排序（据竞品评论区与论坛反馈归纳）：
1. 能不能跑起来（免 root / 免调试模式 / 免虚拟声卡）
2. 延迟是否可接受（听音乐/看视频可容忍 100ms 级，游戏不可）
3. 音质是否被压坏
4. 是否要钱 / 是否有广告 / 是否上传数据
5. 是否支持自己的平台组合

---

## 2. 竞品对比

| 项目 | 主方向 | 平台 | 许可 / 价格 | 依赖前置 | 延迟（宣称） | 加密 |
|---|---|---|---|---|---|---|
| **AudioRelay** | 双向（手机↔电脑） | Windows / macOS / Linux / Android | 专有，freemium（约 $1/月解锁多设备等） | 无需 ADB；支持 Wi-Fi 与 USB | 40–50 ms | 未宣称 |
| **SoundWire** | 电脑 → Android（单向） | 服务端 Windows/Linux/树莓派，客户端 Android | 专有，免费版+付费版 | 无 | 明显可感 | 未宣称 |
| **sndcpy** | Android → 电脑 | 桌面全平台 | Apache-2.0 | **需 ADB + USB 调试 + VLC** | 依赖 VLC 缓冲 | 无 |
| **scrcpy** v4.x | Android → 电脑（含镜像） | 桌面全平台 | Apache-2.0 | **需 ADB + USB 调试**；Android 11+ | 35–70 ms（默认 buffer 50 ms） | 无（ADB 通道） |
| **AudioShare** | Windows → Android | Windows / Android | 开源 | USB 或 Wi-Fi | ~50 ms | 无 |
| **Shairport4w** | iOS → Windows（AirPlay 接收） | Windows | 开源 | 依赖 AirPlay 协议栈 | 65–200 ms | AirPlay 自带 |
| **SoundLink** | **手机 → 电脑** + 电脑 ↔ 电脑 | Windows ✅ / Android ✅ 实测；macOS 部分、Linux/iOS 在建 | **核心 MIT 开源 + Pro 增强闭源（open-core）** | 无需 ADB / root / 虚拟声卡 | 目标 100 ms 级（Jitter 默认 80 ms） | **ChaCha20-Poly1305 端到端** |

### 结论要点

- **方向错位是最大机会**：SoundWire、AudioShare 是「电脑 → 手机」，与本项目场景相反；真正做「手机 → 电脑」的只有 AudioRelay（专有收费）与 sndcpy/scrcpy（必须开 USB 调试）。
- **零前置条件是差异化护城河**：sndcpy/scrcpy 要求 USB 调试，普通用户门槛高；SoundLink 只需装 App + 输配对码。
- **加密是空白位**：调研范围内没有竞品宣称音频面加密。SoundLink 用 ChaCha20-Poly1305 + X25519 + Ed25519，且私钥存 OS keyring、零遥测。
- **iOS 是全行业缺口**：AudioRelay、SoundWire 均无 iOS 发送端（SoundWire 官方说明因平台限制无法上架）。现有替代路径只有 AirPlay（Shairport4w）。SoundLink 的 ReplayKit 路线一旦真机验收通过即为稀缺能力。

---

## 3. SoundLink 的优势与劣势（对外必须诚实列出）

### 优势
1. 手机 → 电脑方向 + PC↔PC 互传，一套工具覆盖。
2. 无需 ADB / USB 调试 / root / 越狱 / 虚拟声卡。
3. 音频面端到端加密，局域网内闭环，无任何遥测上报。
4. 核心 MIT 开源、无广告、无订阅（open-core：Pro 自动化增强闭源买断）。
5. 参数可控：Opus 码率、Jitter 档位、桌面音量运行时可调。
6. 桌面端体验完整：mDNS 自动发现、8 位配对码、信任持久化、断线重连、托盘、开机自启。

### 劣势（首发即需在 README/Release 注明）
1. 仅实测 **Android → Windows**、**Windows → Windows**；macOS 接收未实测、macOS 发送未实装、Linux 未实装、iOS 待真机。
2. 无 USB 传输模式（AudioRelay/scrcpy/AudioShare 均有，Wi-Fi 差时是硬伤）。
3. 单发送端对单接收端，无多接收端广播。
4. 无麦克风回传 / 无反向通道。
5. 延迟不适合游戏与连麦。
6. 桌面 UI 仅中文。
7. 安装包未代码签名，Windows 会触发 SmartScreen。
8. DRM 保护内容不可采（受系统策略约束，不绕过）。

---

## 4. GitHub 文案卖点（可直接用于 About / Release Notes / 社区帖）

一句话（About，≤ 120 字符）：

> Stream your phone's audio to your PC over LAN — encrypted, no root, no ADB, no virtual sound card. Opus / MIT.

中文一句话：

> 把手机正在播放的声音通过局域网送到电脑的耳机与声卡上：加密传输、免 root、免 ADB、免虚拟声卡。

三条主打差异（顺序即重要性）：
1. **不用开发者模式**：装上就能配对，不像 sndcpy/scrcpy 要 USB 调试。
2. **默认加密**：音频面 ChaCha20-Poly1305，密钥存系统钥匙串，零遥测。
3. **核心真开源**：MIT，无订阅无广告，参数全部可调（Pro 增强离线买断）。

建议 Topics：`audio-streaming` `lan` `opus` `tauri` `rust` `flutter` `android` `windows` `headphones` `audio-relay` `low-latency` `encrypted`

---

## 5. 首发推广渠道

| 渠道 | 说明 | 备注 |
|---|---|---|
| GitHub Release（Pre-release） | 一切推广的落地页 | 先完成 OSL-K3 |
| GitHub Topics + 社交预览图 | 自然搜索流量 | OSL-M2 |
| Reddit r/androidapps、r/opensource、r/software | 竞品讨论最集中处 | 需附截图/录屏，实话说明实测范围 |
| Hacker News「Show HN」 | 关注加密与开源属性 | 建议等 macOS 或 iOS 之一可用再发，避免一次性烧掉机会 |
| V2EX / 少数派 / 酷安 | 中文用户，桌面端中文 UI 正合适 | 首发主力 |
| Awesome 类清单 PR | 长尾流量 | 有 Release 后再提 |

节奏建议：**先 GitHub Release + 中文社区 → 收一轮兼容性反馈修完 → 再 Reddit / HN 英文推广**（届时 `README.en.md` 与 UI i18n 应已就位）。

---

## 6. 功能路线的市场优先级（据本次调研调整）

| 缺口 | 市场重要性 | 对应任务 |
|---|---|---|
| iOS 发送端真机验收 | **最高**（全行业缺口） | `docs/First/12-plan.md` 阶段 2 |
| macOS 接收端实测 | 高（Mac + 好耳机用户密集） | release-readiness G3 |
| UI 英文 i18n | 高（英文推广前置） | release-readiness I3 |
| USB 传输模式 | 中（Wi-Fi 差时的唯一退路） | 未规划，建议列入 v1.x |
| Linux 输出 | 中（开源社区期待） | release-readiness G2 |
| 多接收端 | 低 | 未规划 |

---

## 7. 关联文档

- 发布总览与阶段表：[`00-launch-overview.md`](./00-launch-overview.md)
- 产品就绪度：[`../release-readiness/00-release-overview.md`](../release-readiness/00-release-overview.md)
- 延迟设计依据：[`../../First/06-latency-experience.md`](../../First/06-latency-experience.md)
- 安全模型：[`../../../SECURITY.md`](../../../SECURITY.md)、[`../../First/05-pairing-security.md`](../../First/05-pairing-security.md)
- 平台合规边界：[`../../First/08-platform-notes.md`](../../First/08-platform-notes.md)
