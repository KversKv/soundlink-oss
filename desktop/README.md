# desktop — 桌面端（Tauri 2 + Rust）

- `src-tauri/` — Rust 核心（网络/音频/配对/设备/配置/日志）+ Tauri 命令
- `ui/` — 前端界面（React + TypeScript）

第一版为 **Receiver**：mDNS 广播 → 显示配对码 → UDP 接收 → 解密/重排/JitterBuffer/解码/时钟校正 → 输出到选定音频设备。
后续增加 **Sender**（WASAPI Loopback / ScreenCaptureKit）实现双电脑互传。

## 待办（进入阶段 1 时）
运行 `tauri init` 生成完整脚手架（`Cargo.toml`、`tauri.conf.json`、前端脚手架），再按本目录结构填充实现。

详见 `docs/First/02-architecture.md`、`docs/First/10-project-structure.md`。
