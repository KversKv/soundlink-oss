# 08 · 平台能力与合规边界（Platform Notes）

## 1. 关于“手机端模拟音频输出”的现实边界

用户期望“在手机侧模拟一个音频输出，把所有音频流转”。**在合规、可上架前提下，手机端无法创建全局虚拟声卡**，因此采用官方系统级采集能力近似实现：

- iOS：ReplayKit 屏幕广播采集 `.audioApp`。
- Android：MediaProjection + AudioPlaybackCapture。

这两者是最接近“全局音频流转”的**合规**方案，但**不等于**捕获系统所有音频。

## 1b. 移动端应用分层（Flutter 主 App + 原生采集）

移动端采用「Flutter 主 App + 原生采集组件」分层混合架构（决策见 07 §6）：

| 部分 | 技术 | 职责 | 进程 |
|---|---|---|---|
| 主 App | Flutter | 配对/设备发现/设置/广播引导 UI、发起授权 | 主进程 |
| iOS 采集 | 原生 Swift（Broadcast Upload Extension） | 采集 → Opus 编码 → 加密 → UDP 发送 | 独立 Extension 进程 |
| Android 采集 | 原生 Kotlin（前台 Service） | 采集 → Opus 编码 → 加密 → UDP 发送 | 前台 Service |

跨进程约束：
- **Flutter 不进入采集组件**：Extension/Service 保持原生轻量，规避 §2/§3 的内存与合规限制。
- 主 App 与采集组件经 **iOS App Groups 共享容器 / Android Service IPC** 传递配置（对端地址、密钥、开关状态）。
- **音频数据不回传主 App**：采集组件内直接编码发送，避免跨进程搬运音频造成延迟与内存压力。

## 2. iOS

| 能力 | 可否 | 说明 |
|---|---|---|
| 全局虚拟声卡 | 否 | App Store 不允许 |
| 后台静默捕获全部音频 | 否 | 权限限制 |
| ReplayKit 屏幕广播采集音频 | 是 | 官方能力，可上架 |
| AirPlay Receiver | 不建议 | 授权/兼容风险 |
| 越狱/私有 API | 不建议 | 商业化风险高 |

注意事项：
- 用户需从**控制中心 → 屏幕录制 → 选择本 App 的 Broadcast Extension → 开始广播**。主 App 需做良好引导。
- DRM、受保护内容、系统通话音频等**可能无法采集**。
- Broadcast Extension 有**内存与生命周期限制**：内部逻辑必须轻量（不放复杂 UI、不做大缓存、不引入 WebRTC 等重依赖）。

## 3. Android

| 能力 | 可否 | 说明 |
|---|---|---|
| 全局虚拟声卡 | 否（普通应用） | 需特殊权限，非上架路线 |
| AudioPlaybackCapture 采集应用音频 | 是（API 29+） | 官方能力 |
| 采集任意应用 | 部分 | 应用可拒绝被捕获；受保护内容不可采 |

注意事项：
- 需**前台 Service + 用户授权**（MediaProjection 弹窗）。
- 需在通知栏展示采集状态（合规要求）。
- 采集范围受 `AudioAttributes` / 应用声明影响，文案需明确“支持大部分普通应用音频”。

## 4. Windows（桌面）

- 输出：WASAPI Render（`IAudioClient3` / `IAudioRenderClient` / `IMMDeviceEnumerator`），支持设备枚举、低延迟、音量、插拔处理。
- 采集（Sender，后续）：WASAPI Loopback 采集系统正在播放的音频。

## 5. macOS（桌面）

- 输出：CoreAudio / AudioUnit，设备枚举与低延迟输出。
- 采集（Sender，后续）：优先 ScreenCaptureKit Audio Capture，而非一开始就做虚拟声卡（虚拟声卡需系统扩展/AudioServerPlugIn，签名/公证复杂）。

## 6. Linux（后续）

- 优先级：PipeWire > PulseAudio > ALSA。
- 第一版可不做。

## 7. 产品文案建议

> 本软件基于系统官方采集能力，支持大部分普通应用音频；受 DRM 或应用限制的内容可能无法流转。为获得更好音画同步，建议电脑端使用有线/USB/2.4G 低延迟耳机。
