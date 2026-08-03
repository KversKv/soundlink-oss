# 安全策略 · Security Policy

## 支持的版本

项目处于早期阶段，仅对**最新提交与最新 Release** 提供安全修复。

| 版本 | 是否支持 |
|---|---|
| `main` 最新提交 | ✅ |
| 最新 Release | ✅ |
| 更早的 Release / tag | ❌ |

---

## 报告漏洞

**请不要在公开 Issue / Discussion / PR 中披露安全漏洞。**

推荐渠道（任选其一）：

1. GitHub **Private vulnerability reporting**：仓库 → Security → Report a vulnerability（首选）。
2. 若上述渠道不可用，可开一个**不含任何技术细节**的 Issue，标题写「Security contact request」，等待维护者联系。

报告请尽量包含：

- 影响的组件（桌面接收端 / 桌面发送端 / Android / iOS / 共享协议层）
- 版本或 commit hash
- 复现步骤与最小复现环境（网络拓扑、是否同一局域网等）
- 实际影响（可窃听音频 / 可伪造设备 / 可绕过配对 / 可远程崩溃 / 可任意代码执行等）
- 可能的修复建议（可选）

处理约定：

- 我们会确认收到并给出初步评估结论。
- 修复完成后在 Release Notes 与 [`CHANGELOG.md`](CHANGELOG.md) 中致谢（如你希望匿名请说明）。
- 请在修复发布前不要公开细节。

---

## 安全模型（威胁边界）

SoundLink 的安全设计假设与边界，报告漏洞前请对照，以判断是否属于「已知设计限制」：

**设计保证**

- 音频面使用 **ChaCha20-Poly1305** AEAD 加密，会话密钥通过 **X25519** 协商 + **HKDF-SHA256** 派生。
- 设备身份使用 **Ed25519**，配对使用 **HMAC-SHA256** 配对码证明，防止中间人被动接管。
- 设备私钥与固定配对码存放在 **操作系统 keyring**（Windows Credential Manager / macOS Keychain），不落明文文件。
- 配对码错误尝试有次数限制与锁定，防暴力枚举。
- **零遥测**：不向任何外部服务器上报数据，详见 [`docs/privacy.md`](docs/privacy.md)。

**明确不在保护范围（已知限制，非漏洞）**

- **仅局域网可信边界**：不设计用于公网直连；把设备暴露到不可信网络属于超出设计的使用方式。
- **同机恶意进程**：无法防御已在同一台机器上以同等权限运行的恶意程序（可读取 keyring / 注入进程）。
- **物理接触**：无法防御可物理访问已解锁设备的攻击者。
- **DRM 绕过**：项目不绕过系统 DRM 策略；「某应用音频采集为静音」是预期行为而非漏洞。
- **发送端流量存在**：UDP 音频包内容加密，但通信元数据（源/目的 IP、端口、包时序与长度）不隐藏，不提供流量分析抗性。
- **无多接收端授权隔离**：当前一个发送端对应一个接收端，未设计多租户权限模型。

---

## 相关文档

- 配对与安全设计：[`docs/First/05-pairing-security.md`](docs/First/05-pairing-security.md)
- 协议规格：[`docs/First/04-protocol.md`](docs/First/04-protocol.md)
- 隐私政策：[`docs/privacy.md`](docs/privacy.md)
