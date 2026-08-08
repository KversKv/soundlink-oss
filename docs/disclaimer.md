# SoundLink 免责声明

> 生效日期：2026-08-08
> 适用范围：SoundLink 桌面端（Windows）、移动端（iOS / Android）、官方网站及相关源代码仓库 KversKv/soundlink-oss。

SoundLink 是一款面向头戴式耳机用户的**局域网音频流转**软件，以 MIT 许可证开源发布。使用本软件即表示你已阅读、理解并接受本声明的全部内容。

---

## 1. 按「现状」提供

SoundLink 按「现状」（AS IS）提供，不附带任何明示或默示的保证，包括但不限于对适销性、特定用途适用性与不侵权的保证。作者不对因使用或无法使用本软件而产生的任何直接、间接、附带或后果性损害承担责任。完整条款以仓库根目录 [`LICENSE`](../LICENSE)（MIT）为准。

---

## 2. 实测范围限制

当前**仅实测通过**以下组合：

- Android 手机 → Windows 电脑
- Windows 电脑 → Windows 电脑

其他组合（macOS、Linux、iOS 等）处于「代码就绪 / 未实测」或「未实装」状态，请勿按「可用」预期。平台支持状态以 [`README.md`](../README.md) 功能矩阵与官网「平台支持」分区为准。

---

## 3. 安装包未代码签名

Release 安装包**未购买代码签名证书**，Windows SmartScreen 首次运行会提示「未知发布者」，属预期行为。下载后请先比对 Release 页提供的 SHA256 校验值再运行：

```powershell
Get-FileHash .\SoundLink-Setup.exe -Algorithm SHA256
```

请仅从官方仓库 [KversKv/soundlink-oss Releases](https://github.com/KversKv/soundlink-oss/releases) 获取安装包；从其他渠道获得的安装包不在本声明与校验范围内。

---

## 4. DRM 受保护内容不可采

Windows WASAPI Loopback、iOS ReplayKit、Android MediaProjection 均为操作系统官方采集能力，**受系统 DRM 策略约束**：

- 部分受 DRM 保护的应用音频（如部分流媒体、受保护的视频内容）无法被采集，表现为无声或静音。
- 该限制属操作系统层面的安全设计，SoundLink **无法绕过，也不试图绕过**。

---

## 5. 延迟与适用场景

SoundLink 的延迟目标是**听音乐与看长视频**（100 ms 级），**不适合**游戏、连麦等实时互动场景；短视频场景可能感知轻微延迟。请勿将其用于对延迟敏感的用途（如实时监控、演出返听）。

---

## 6. 合规使用义务

- 请仅采集与传输**你有权使用**的音频内容。用户应自行遵守所在地区的版权法规与相关平台的服务条款；因不当使用产生的法律责任由用户自行承担。
- SoundLink 仅在**用户主动发起**时采集本机音频，且音频仅在用户所在局域网内传输（详见 [`docs/privacy.md`](./privacy.md)）。请勿将其用于未经他人同意的监听或录制。

---

## 7. 网络环境限制

SoundLink 仅工作在局域网内，不支持公网与 NAT 穿透。路由器的 AP 隔离、访客网络、企业网策略可能阻断设备发现与连接，这属于网络环境限制而非软件缺陷。

---

## 8. 第三方组件

SoundLink 依赖 libopus、Tauri、tokio 等开源组件，其功能与许可由各自项目决定，完整清单见 [`docs/privacy.md`](./privacy.md) 第 6 节。这些组件按其各自许可证「按现状」提供。

---

## 9. 声明的更新

本声明可能随版本演进更新，更新后将在本文件与仓库中公示。继续使用本软件即视为接受更新后的声明。
