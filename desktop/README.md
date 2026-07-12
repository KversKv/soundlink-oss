# desktop — 桌面端（Tauri 2 + Rust + React/TS）

- `src-tauri/` — Rust 核心（网络/音频/配对/设备/配置/日志）+ Tauri 命令
- `ui/` — 前端界面（React + TypeScript）

支持 **Receiver** 与 **Sender** 两种角色：

- **Receiver**：mDNS 广播 → 显示配对码 → UDP 接收 → 解密/重排/JitterBuffer/解码/时钟校正 → 输出到选定音频设备。
- **Sender**（Windows）：WASAPI Loopback 采集系统音频 → Opus 编码 → 加密 → UDP 发送到指定 Receiver，支持 backoff 重连。
- **Sender**（macOS）：ScreenCaptureKit 采集占位，未实装。

---

## 依赖环境

- Rust 1.80+（stable，MSVC 工具链）
- Node.js 18+ 与 npm/pnpm
- Windows：WebView2 Runtime（Windows 10+ 默认随附）、MSVC Build Tools
- macOS / Linux：见 [Tauri 2 前置依赖](https://tauri.app/start/prerequisites/)
- libopus 构建依赖：cmake + C 编译器（vendored libopus 1.5）

---

## Feature 矩阵

| Feature | 用途 | 默认 | 备注 |
|---|---|---|---|
| `default` | 仅核心库，无 Tauri 外壳 | ✅ | 用于 `cargo test` 与 examples |
| `opus` | 启用真实 libopus（vendored） | 否 | 生产环境必须，否则解码回退 passthrough 产生噪声 |
| `wasapi` | Windows WASAPI Loopback 采集 | 否 | 仅 Windows；非 Windows 平台为 no-op |
| `tauri_app` | Tauri 应用外壳（自动启用 `opus` + `wasapi`） | 否 | 生产构建必选 |

`tauri_app` 自动聚合：`tauri` / `tauri-build` / `dirs` / `tauri-plugin-opener` / `tauri-plugin-autostart` / `tauri-plugin-single-instance` / `tauri-plugin-window-state` / `opus` / `wasapi`。

---

## 构建命令

### 开发模式

```powershell
cd desktop
npm install
npm run tauri dev
```

### 生产构建

```powershell
cd desktop
npm run tauri build
# 产物：src-tauri/target/release/bundle/
```

### 仅构建 Rust 核心（无 Tauri 外壳，用于 examples / 单测）

```powershell
cd desktop/src-tauri
cargo build                              # 核心库
cargo test --features opus               # 含 libopus roundtrip
cargo test --features wasapi             # 含 WASAPI 单测
cargo run --example loopback_sender      # 440Hz 环回自测
cargo run --example phase4_loopback      # 弱网自测
cargo run --example phase5_loopback      # 双向环回自测
```

> 生产构建必须用 `--features tauri_app`（由 `npm run tauri build` 自动注入）。

---

## 目录结构

```
desktop/
├── src-tauri/
│   ├── src/
│   │   ├── audio/         # 采集（WASAPI/macos 占位）+ 输出（cpal）+ 重采样 + Jitter
│   │   ├── commands/      # Tauri 命令 + 托盘
│   │   ├── config/        # app_config.json + keyring
│   │   ├── device/        # DeviceIdentity（Ed25519）
│   │   ├── logging/       # 按日轮转文件日志
│   │   ├── network/       # UDP 接收 + TCP 控制服务器 + mDNS
│   │   ├── pairing/       # 配对码 + 密钥交换
│   │   ├── receiver.rs    # ReceiverEngine
│   │   ├── sender.rs      # SenderEngine（含 backoff 重连）
│   │   ├── constants.rs   # 端口/音频基线/锁定时长
│   │   └── main.rs        # Tauri Builder + 插件注册
│   ├── examples/          # 环回自测（loopback_sender / phase4 / phase5）
│   ├── capabilities/      # Tauri 2 权限配置
│   ├── build.rs           # 注入 BUILD_DATE
│   └── Cargo.toml
└── ui/
    ├── src/
    │   ├── App.tsx                  # 主界面（角色切换/接收/发送/设置）
    │   ├── components/
    │   │   ├── Onboarding.tsx       # 首次使用 3 步引导
    │   │   ├── SettingsPanel.tsx    # 设置面板（启动/关闭/设备/日志/关于）
    │   │   └── CloseDialog.tsx
    │   └── utils/
    └── package.json
```

---

## 关键能力

| 能力 | 实现 |
|---|---|
| 单实例锁定 | `tauri-plugin-single-instance`（首次注册，二次启动聚焦既有窗口） |
| 窗口大小/位置记忆 | `tauri-plugin-window-state` |
| 开机自启动 | `tauri-plugin-autostart` |
| 设备身份 | Ed25519 私钥存 OS keyring；device_id.txt 落盘；加载失败 `try_persist_temp` + UI 警告 |
| 固定配对码 | OS keyring 存储；不落 JSON |
| 配对失败锁定 | 5 次错误后锁定 60s；UI 倒计时 |
| 网络断开重连 | Sender backoff `[5s, 10s, 30s]` 三档；`sender-state-changed` 事件 |
| 退出清理 | `cleanup_before_quit` 异步清理 + 1s timeout |
| 日志 | 按日轮转 `soundlink.log.YYYY-MM-DD`；不落密钥/配对码明文 |
| DRM 提示 | 首次开启发送模式弹窗；`sender_drm_hint_seen` 持久化 |

---

## 关联文档

- 架构：[`docs/First/02-architecture.md`](../docs/First/02-architecture.md)
- 项目结构：[`docs/First/10-project-structure.md`](../docs/First/10-project-structure.md)
- 实现规格：[`docs/First/11-implementation-spec.md`](../docs/First/11-implementation-spec.md)
- 用户使用指南：[`docs/user/desktop-guide.md`](../docs/user/desktop-guide.md)
- 桌面端开发环境：[`docs/user/02-dev-env-desktop.md`](../docs/user/02-dev-env-desktop.md)
