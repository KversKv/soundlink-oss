# desktop/ui — Tauri 前端（React + TypeScript）

- `src/pages/` — 页面：接收主页（配对码/连接状态/设备选择）、设备管理、设置
- `src/components/` — 复用组件：设备列表、配对码卡片、网络质量/延迟指示
- `src/stores/` — 状态管理：连接状态、设备、统计（订阅 Tauri 事件/命令）

通过 Tauri command（见 `src-tauri/src/commands`）与 Rust Core 交互。

## 待办（进入阶段 1 时）
由 `tauri init` 生成前端脚手架（Vite + React + TS）后填充。
