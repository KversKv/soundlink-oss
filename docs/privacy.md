# SoundLink 隐私政策

> 生效日期：2026-07-12
> 适用范围：SoundLink 桌面端（Windows）、移动端（iOS / Android）及相关源代码仓库 KversKv/soundlink-oss。

SoundLink 是一款面向头戴式耳机用户的**局域网音频流转**软件。本政策说明我们采集的数据范围、本地数据清单、网络传输机制与第三方组件。

---

## 1. 数据采集范围

SoundLink **仅在用户主动开启发送模式时**采集本机系统音频，且采集行为完全由用户发起：

- **桌面发送端（Windows）**：通过 WASAPI Loopback 采集本机输出音频，由用户在 UI 中点击「开始发送」后启动。
- **iOS 发送端**：通过 ReplayKit Broadcast Extension 采集系统音频，由用户在系统控制中心主动启动广播。
- **Android 发送端**：通过 MediaProjection 采集应用音频，由用户在前台通知授权后启动。

**采集的音频仅在用户所在的局域网内传输**，目的地址为用户指定的接收端设备，**不离开局域网、不上传到任何服务器**。

接收端不采集任何音频，仅接收并播放来自发送端的音频流。

---

## 2. 本地数据清单

SoundLink 在本机存储以下数据，均位于各操作系统的应用数据目录，**不主动上传任何数据**：

| 数据 | 位置（桌面端示例） | 用途 | 敏感性 |
|---|---|---|---|
| 设备身份（Ed25519 私钥/设备 ID） | OS keyring + `app_config/device_id.txt` | 设备身份与配对认证 | 高（私钥存 keyring） |
| 固定配对码 | OS keyring | 免重复输入配对码 | 高 |
| 信任设备列表 | `app_config/trust_store.json` | 已配对设备的公钥与别名 | 中 |
| 应用配置 | `app_config/app_config.json` | 设备名/角色/音频参数/启动项 | 低 |
| Pro 授权码 | OS keyring + `app_config/license.key` | 离线授权校验（Ed25519 签名） | 低（授权凭据，非安全密钥） |
| 日志文件 | `app_config/logs/soundlink.log.YYYY-MM-DD` | 故障排查；按日轮转；**不含密钥/配对码明文** | 低 |
| Crash 报告 | 系统默认（无主动收集） | 无内置 crash 上报机制 | — |

iOS 端使用 Keychain，Android 端使用 Keystore / SharedPreferences 存储对应的身份与信任数据。

日志遵循项目规则：**密钥、配对码、会话密钥不落日志**（参见 `.trae/rules/project-rules.md` 硬红线）。

---

## 3. 网络传输声明

SoundLink 的全部网络通信均发生在**用户所在的局域网内**：

- **音频流**：UDP，端口 `47811`（`DEFAULT_AUDIO_PORT`），携带 AEAD 加密的 Opus 音频帧。
- **控制通道**：TCP，端口 `47810`（`DEFAULT_CONTROL_PORT`），承载配对握手、设备发现、stats 回传等 JSON 消息。
- **设备发现**：mDNS 广播 `_soundlink._udp.local`，仅在同一局域网内可见。

**加密协议栈**（详见 [`docs/First/05-pairing-security.md`](./First/05-pairing-security.md)）：

| 用途 | 算法 |
|---|---|
| 对称加密（音频帧 / AEAD） | ChaCha20-Poly1305 |
| 临时密钥协商 | X25519 |
| 设备身份 / 签名 | Ed25519 |
| 密钥派生 | HKDF-SHA256 |
| 配对证明 | HMAC-SHA256 |

首次配对时用户需在接收端输入配对码完成身份交换；配对成功后双方互存公钥，后续连接跳过配对码直接协商临时会话密钥。

**SoundLink 不主动向任何公网服务器发起连接**；除用户所在的局域网外，无任何外发数据流。

---

## 4. 遥测声明

**SoundLink 不收集任何遥测数据**。

- 无使用统计、无崩溃上报、无匿名化分析。
- 不内置任何第三方分析 SDK。
- 应用不主动向作者或第三方服务器报告任何信息。

如需排查问题，用户可手动在「设置 → 日志」打开日志目录并提供给开发者；日志不含密钥明文。

---

## 4.1 Pro 授权校验（完全离线）

桌面端 Pro 授权**完全在本地校验，不联网、不上传任何信息、无激活服务器**：

- **授权码**：`SLPRO-…` 形式，内含 Ed25519 签名的授权载荷。校验用内置公钥在本机离线验签，**全程无任何网络请求**。
- **设备指纹**：用于把授权绑定到本机。指纹 = `base32(SHA256("soundlink-fp-v1" ‖ 机器标识 ‖ 设备 ID))` 取前 10 位，是**单向哈希短码**，不含任何可还原的隐私信息，也不会被上传。
  - 机器标识读取自系统公开位置（Windows 注册表 `MachineGuid` / macOS `IOPlatformUUID` / Linux `/etc/machine-id`），仅参与本机哈希计算，**不出本机**。
- **存储**：授权码存于 OS keyring，兜底为配置目录 `license.key`（与 `app_config.json` 同目录）。
- **无到期回连、无心跳上报**：授权为一次性买断，校验不含时间回连逻辑；校验失败只会降级为免费版，绝不阻止启动或中断音频。

---

## 5. DRM 受保护内容限制

Windows WASAPI Loopback、iOS ReplayKit、Android MediaProjection 均为操作系统提供的官方采集能力，**受系统 DRM 策略约束**：

- 部分受 DRM 保护的应用音频（如部分流媒体服务、受保护的视频内容）可能无法被采集，表现为无声或静音。
- 该限制属于操作系统层面的安全设计，SoundLink **无法绕过**，也不试图绕过。
- 桌面端首次开启发送模式时会主动提示「部分受 DRM 保护的应用音频可能无法采集，属系统限制」。

---

## 6. 第三方组件

SoundLink 依赖以下主要开源组件，其许可证与数据处理行为由各自项目决定：

| 组件 | 用途 | 许可证 |
|---|---|---|
| libopus | 音频编解码 | BSD-3-Clause |
| cpal | 跨平台音频输出 | MIT/Apache-2.0 |
| tokio | Rust 异步运行时 | MIT |
| Tauri 2 | 桌面端框架 | MIT/Apache-2.0 |
| windows crate | Windows WASAPI 绑定 | MIT/Apache-2.0 |
| chacha20poly1305 / x25519-dalek / hkdf / ed25519-dalek | 加密协议栈 | MIT/Apache-2.0 |
| mdns-sd | 局域网设备发现 | MIT/Apache-2.0 |
| keyring | OS keyring 访问 | MIT/Apache-2.0 |
| ReplayKit / MediaProjection | iOS/Android 系统采集 | 平台 SDK |

以上组件均在用户本机运行，不引入额外的网络上报行为。

---

## 7. 权限说明

SoundLink 在各平台申请的权限仅用于实现音频流转功能：

- **Windows 桌面端**：网络访问（局域网 UDP/TCP）；不申请麦克风权限（采集的是系统输出而非麦克风）。
- **iOS**：本地网络权限、ReplayKit 广播权限。
- **Android**：前台 Service（媒体投影）、`FOREGROUND_SERVICE_MEDIA_PROJECTION`、本地网络。

---

## 8. 数据删除

用户可通过以下方式清除 SoundLink 存储的本机数据：

- **卸载应用**：移动端卸载会清除应用数据目录；桌面端卸载后 `app_config/` 目录可能残留，可手动删除。
- **桌面端「设置 → 日志」**：可打开日志目录手动清理历史日志。
- **重置配对**：在接收端清空信任列表后，已配对设备的信任关系失效，需重新配对。

---

## 9. 联系方式

- 问题反馈：[GitHub Issues](https://github.com/KversKv/soundlink-oss/issues)
- 源代码仓库：https://github.com/KversKv/soundlink-oss

---

## 10. 政策变更

本政策随项目迭代更新；实质性变更会通过 GitHub 仓库提交记录公示，不另行单独通知。用户可关注仓库 CHANGELOG 或 `docs/privacy.md` 的提交历史。

---

## 关联文档

- 安全设计：[`docs/First/05-pairing-security.md`](./First/05-pairing-security.md)
- 协议规格：[`docs/First/04-protocol.md`](./First/04-protocol.md)
- 平台合规：[`docs/First/08-platform-notes.md`](./First/08-platform-notes.md)
- 工程规则：[`.trae/rules/project-rules.md`](../.trae/rules/project-rules.md)
