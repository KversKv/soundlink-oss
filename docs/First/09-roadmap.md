# 09 · 开发路线（Roadmap）

## 阶段 1 · 桌面端接收器 MVP
**目标**：桌面端可启动接收服务、显示配对码、接收测试音频流、输出到指定设备。
- Tauri UI 骨架
- Rust UDP Server
- Opus 解码
- WASAPI / CoreAudio 输出
- 简单 Jitter Buffer

**验收**：本地发一个测试 Opus/UDP 流，桌面能选择设备并稳定播放。

## 阶段 2 · 移动端采集 MVP（Flutter 主 App + 原生采集）
**目标**：手机可开启广播/授权，采集应用音频，编码 Opus，发送到桌面播放。UI 用 Flutter 统一，采集组件保持原生（决策见 07 §6、08 §1b）。
- Flutter 主 App：配对/设备/设置/广播引导界面（iOS/Android 共用一套）
- iOS 采集（原生 Swift Extension）：ReplayKit、CMSampleBuffer 解析、AudioBufferList→PCM、Opus 编码、UDP 发送；经 App Groups 与主 App 通信
- Android 采集（原生 Kotlin Service）：MediaProjection、AudioPlaybackCapture、AudioRecord→PCM、Opus 编码、UDP 发送；经 Service IPC 与主 App 通信

**验收**：手机播放音乐，桌面端能听到，端到端可用。

## 阶段 3 · 配对与设备发现
**目标**：桌面端自动被手机发现，输入配对码建立信任，下次自动连接。
- Bonjour / mDNS / NSD
- 配对码 + 密钥协商（第一版 X25519 + HMAC）
- iOS Keychain / Android Keystore / 桌面 trust store

**验收**：无需手输 IP，配对一次后可自动重连。

## 阶段 4 · 体验优化
**目标**：降延迟、降卡顿、增强弱网稳定性、改善音画同步。
- 自适应 Jitter Buffer
- 丢包统计 + Opus PLC
- 码率自适应
- 桌面输出 buffer 调优
- 延迟估算与 UI 展示

## 阶段 5 · 桌面发送端（双电脑互传）
**目标**：Windows/macOS 电脑作为 Sender，支持电脑到电脑流转。
- Windows WASAPI Loopback
- macOS ScreenCaptureKit
- 统一 Sender 抽象层

## 阶段 6（可选）· 扩展
- Linux（PipeWire）
- 多接收端、安全升级到 PAKE、二维码配对等

## 里程碑与平台优先级

| 阶段 | iOS | Android | Windows | macOS |
|---|:--:|:--:|:--:|:--:|
| 1 Receiver MVP | - | - | ✅ | ✅ |
| 2 采集 MVP | ✅ | ✅ | - | - |
| 3 配对发现 | ✅ | ✅ | ✅ | ✅ |
| 4 体验优化 | ✅ | ✅ | ✅ | ✅ |
| 5 桌面 Sender | - | - | ✅ | ✅ |
