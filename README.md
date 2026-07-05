# SoundLink

面向头戴式耳机用户的**局域网音频流转**软件：手机（iOS/Android）音频 → 局域网 → 电脑音频设备；支持电脑到电脑互传。

## 目录

- `mobile/` — 移动发送端（iOS + Android）
- `desktop/` — 桌面端（Tauri 2 + Rust，Receiver + 后续 Sender）
- `shared/` — 跨端协议与常量
- `docs/First/` — 设计文档（从 `SoundLinkStructrue.md` 开始）

## 从这里开始

1. 阅读 [`docs/First/SoundLinkStructrue.md`](docs/First/SoundLinkStructrue.md)（顶层导航）
2. 协作规则见 [`AGENTS.md`](AGENTS.md) 与 [`.trae/rules/project-rules.md`](.trae/rules/project-rules.md)

## 当前状态

规划完成，仓库为**骨架 + 占位说明**，尚未进行脚手架初始化与可运行实现。开发阶段见 [`docs/First/09-roadmap.md`](docs/First/09-roadmap.md)。
