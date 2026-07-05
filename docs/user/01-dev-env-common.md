# 01 · 开发环境搭建 · 通用前置

适用于所有平台的通用准备工作。各端专属环境见：
- 桌面端 → [02-dev-env-desktop.md](./02-dev-env-desktop.md)
- iOS → [03-dev-env-ios.md](./03-dev-env-ios.md)
- Android → [04-dev-env-android.md](./04-dev-env-android.md)

## 1. 获取仓库

```bash
git clone <repo-url> SoundLink
cd SoundLink
```

## 2. 仓库结构（速览）

```text
SoundLink/
├── mobile/{ios,android}   # 移动发送端
├── desktop/               # src-tauri (Rust 核心) + ui (前端)
├── shared/                # 跨端协议与常量（单一事实来源）
├── docs/                  # 设计文档 (First/) + 用户文档 (user/)
├── AGENTS.md              # AI 协作说明
└── .gitignore
```

完整职责说明见 [`docs/First/10-project-structure.md`](../First/10-project-structure.md)。

## 3. 通用工具

| 工具 | 用途 | 建议版本 |
|---|---|---|
| Git | 版本控制 | 最新稳定版 |
| Node.js | 桌面前端 / 工具脚本 | LTS（≥ 20） |
| pnpm 或 npm | 前端包管理 | pnpm ≥ 8（推荐）|

安装校验：

```bash
git --version
node --version
```

## 4. 共享层（shared/）

`shared/` 集中定义**协议消息、音频包结构、端口/服务类型/音频参数常量**，是各端一致性的单一来源。

- 修改协议时须同步更新 [`docs/First/04-protocol.md`](../First/04-protocol.md)。
- 禁止在各端散落魔法值；音频基线（48kHz / Stereo / Opus 10ms / 128kbps / 默认 Jitter 80ms）不得随意更改。

## 5. 网络前置要求

- 所有设备处于**同一局域网**（第一版不支持公网 / NAT 穿透）。
- 局域网需允许 mDNS / Bonjour 广播与 UDP 单播。
- 防火墙需放行音频 UDP 端口与控制 TCP/WS 端口（默认端口见 `shared/constants`）。

## 6. 代码规范提示

- Rust 使用 `tracing` 记录日志，核心逻辑禁用 `println!`。
- 密钥 / 配对码禁止明文落日志。
- 提交前跑各端 lint（见对应平台文档）。
