<!-- FT-0014 -->
# P2 阶段 Windows 可验证任务批量实施（2026-07-12）

> 场景：P0/P1 已完成，进入 P2「后续版本优化」阶段。本会话聚焦 Windows 环境可验证的 11 项任务（G4/G5/H1-H4/I2/I4-I7），跨平台实装（G1-G3）与测试基建（H5-H7）留待后续。

## 背景与范围

P2 共 20 个任务（G1-G5 / H1-H7 / I1-I8），工作量大且部分依赖 macOS/Linux 真机验证。经用户确认本次范围：

**本次完成（11 项）**：G4 / G5 / H1 / H2 / H3 / H4 / I2 / I4 / I5 / I6 / I7
**本次跳过（9 项）**：G1 / G2 / G3 / H5 / H6 / H7 / I1 / I3 / I8

## 实现清单

| 任务 | 文件 | 改动概要 |
|---|---|---|
| 前置 | `desktop/src-tauri/Cargo.toml` | `[dev-dependencies]` 加 `tempfile = "3"` |
| H3 | `desktop/src-tauri/src/config/mod.rs` | 末尾追加 13 个单测（default/normalized 6 字段/load_or_default 三分支/save roundtrip/jitter_mode_from_ms） |
| H1 | `desktop/src-tauri/src/commands/mod.rs` | 末尾追加 14 个纯函数单测（parse_role/role_as_str/parse_jitter_mode/nearest_bitrate 11 个 + make_capture_source 3 个） |
| H4 | `desktop/src-tauri/src/audio/capture/wasapi_loopback.rs` | 现有 `#[cfg(test)] mod tests` 内追加 8 个 ring buffer 边界用例 |
| H2 | `desktop/src-tauri/src/commands/tray.rs` | 新增 `CloseDecision` 枚举 + `decide_close_action` 纯函数；重构 `handle_close_requested` 调用纯函数；追加 6 单测 |
| I4 | `desktop/src-tauri/src/receiver.rs` + `network/control_server.rs` | start() 改全量重置 `*ls = LatencyState::default()`；新增 `reset_latency_state()` 公开方法；同 key 重连 else 分支补调用 |
| G5 | `desktop/src-tauri/tauri.conf.json` | bundle 节点追加 `macOS` 子节点（signingIdentity/entitlements/minimumSystemVersion/dmg 配置） |
| G4 | `desktop/src-tauri/Cargo.toml` | wasapi feature 注释补「Windows 端 cargo check --no-default-features --features tauri_app 已验证；macOS/Linux 真机待 G1/G2/G3」 |
| I5 | `desktop/src-tauri/src/sender.rs` + `commands/mod.rs` + `desktop/ui/src/App.tsx` | SenderEngine 新增 `on_pubkey_mismatch` 字段 + `set_on_pubkey_mismatch` 方法；handshake 检测 MITM 时 return Err 前调用回调 emit `pubkey-mismatch`；前端 listen + 模态弹窗（删除并重配对/取消） |
| I2 | `desktop/src-tauri/Cargo.toml` + `main.rs` + `capabilities/default.json` + `App.tsx` | 加 `tauri-plugin-global-shortcut` 依赖；Builder 注册插件 + setup 注册 Ctrl+Shift+P/S；capabilities 加权限；前端 listen `global-shortcut` 事件分发 toggle-role/show-window |
| I6 | `desktop/ui/src/components/SettingsPanel.tsx` | 日志 section 后插入「使用帮助」section（外链文档/反馈 + 快速上手说明） |
| I7 | `desktop/ui/src/components/SettingsPanel.tsx` | 日志 section 加刷新按钮 + 自动刷新开关（5s）+ 关键字过滤输入框 |

## 关键设计决策

### 1. H2 CloseDecision 枚举（非 bool 组合）

未采用 `struct CloseDecision { prevent_close: bool, should_emit: bool }` 组合字段，而是用枚举变体 `Minimize | Quit | Ask`。原因：组合字段会产生「Minimize 但 should_emit=true」这种无效组合，枚举变体天然互斥，副作用在 `handle_close_requested` 中按变体分发，顺序与原代码逐分支对照保持一致。

### 2. I5 安全语义不削弱

**关键决策**：不实现「挂起 handshake 等待用户响应」的复杂状态机。Rust 端在公钥不匹配时仍**立即 return Err**，回调仅作事后告知。前端「删除并重配对」是新的连接流程（用户主动调 `remove_trusted_receiver` + 重新 `start_sender`），不是「继续当前被阻断的连接」。安全语义与 P0 A5 完全一致，仅 UI 体验增强。

### 3. I4 latency_state 全量重置

`LatencyState` 含 10 个字段，原 `start()` 仅显式重置 `first_recv_instant`/`bitrate_start` 两字段，其余字段依赖首次收包时初始化。同 key 重连路径（`control_server.rs` else 分支）跳过 `start()`，所有字段保留旧值。修复方案：`start()` 改用 `*self.latency_state.lock() = LatencyState::default()` 一次性重置全部 10 字段；新增 `reset_latency_state()` 公开方法供同 key 重连路径调用。`LatencyState` derive Default，10 字段默认值与首次启动语义一致。

### 4. H1/H2 测试 feature 门控

`commands/mod.rs` 与 `tray.rs` 文件头 `#![cfg(feature = "tauri_app")]`，测试模块须用 `#[cfg(all(test, feature = "tauri_app"))]`，否则无 feature 编译时整个文件不参与编译，测试无法发现。验证命令必须带 `--features tauri_app`。

### 5. I2 快捷键分发模式

未在 Rust 端 `with_handler` 内直接调用 `set_role`/`show_main_window` 命令，而是 emit `global-shortcut` 事件给前端，由前端 listener 分发。原因：保持 Rust 端插件注册的简洁性，前端已有完善的 role state 管理与命令调用逻辑，避免 Rust 端重复维护 role 切换的 UI 联动。

### 6. I5 on_pubkey_mismatch 回调无死锁风险

emit 在 `return Err` 之前调用，此时 `trust.lock()`（L527-531）已释放；`on_pubkey_mismatch.lock()` 与 `trust.lock` 是不同 Mutex，无嵌套。回调内通过 `app.emit` 走 Tauri 事件总线，非同步阻塞。

## 验证结果

### 单元测试

```
cargo test --features tauri_app
test result: ok. 95 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

本次新增 41 个测试（H3 13 + H1 14 + H4 8 + H2 6），无回归。

### 编译验证

- `cargo check --features tauri_app` ✅ 通过
- `cargo check --no-default-features` ✅ 通过（G4 no-op 验证）
- `npm --prefix desktop/ui run build` ✅ 通过（I5/I2/I6/I7 前端构建）

## 已知边界（待真机/后续验证）

| 任务 | 待验证项 |
|---|---|
| G5 | dmg 实际生成（macOS 真机 `cargo tauri build`） |
| G4 | 非 Windows 平台 `cargo check`（macOS/Linux 真机） |
| I2 | macOS Cmd 键映射（tauri-plugin 默认行为，待真机） |
| I4 | 长时断网重连场景（>1h 多次重连后码率统计归零） |
| I5 | 真机公钥变化模拟（需篡改 trust_store.json 触发） |

## 用户需自行完成部分

- macOS 真机：`cargo tauri build` 验证 dmg 生成 + ScreenCaptureKit 采集实装（G1）
- Linux 真机：PipeWire 输出实装（G2）+ cpal CoreAudio 输出验证（G3）
- 长时压测：1h+ 连续收发记录内存/CPU/丢包率（H7）
- E2E 框架：Playwright + Tauri WebDriver 接入（H5）
- CI 流水线：`.github/workflows/ci.yml` 三平台矩阵（H6）

## 关键文件索引

- `desktop/src-tauri/Cargo.toml` — tempfile dev-dep + global-shortcut 依赖 + wasapi 注释
- `desktop/src-tauri/src/config/mod.rs:234+` — H3 13 单测
- `desktop/src-tauri/src/commands/mod.rs:986+` — H1 14 单测
- `desktop/src-tauri/src/commands/tray.rs:26-45` — CloseDecision 枚举 + decide_close_action
- `desktop/src-tauri/src/audio/capture/wasapi_loopback.rs:454+` — H4 8 ring 边界单测
- `desktop/src-tauri/src/receiver.rs:327` — I4 start() 全量重置
- `desktop/src-tauri/src/receiver.rs:542-545` — I4 reset_latency_state 方法
- `desktop/src-tauri/src/network/control_server.rs:596-600` — I4 同 key 重连补 reset
- `desktop/src-tauri/src/sender.rs:93-95` — I5 on_pubkey_mismatch 字段
- `desktop/src-tauri/src/sender.rs:159-167` — I5 set_on_pubkey_mismatch 方法
- `desktop/src-tauri/src/sender.rs:595-610` — I5 MITM 检测回调
- `desktop/src-tauri/src/commands/mod.rs:675-687` — I5 回调注入
- `desktop/src-tauri/src/main.rs:49-65` — I2 global-shortcut 插件注册
- `desktop/src-tauri/src/main.rs:72-79` — I2 setup 注册快捷键
- `desktop/src-tauri/capabilities/default.json` — I2 权限
- `desktop/src-tauri/tauri.conf.json:48-62` — G5 macOS 节点
- `desktop/ui/src/App.tsx:294-304` — I5 pubkey-mismatch listener
- `desktop/ui/src/App.tsx:305-316` — I2 global-shortcut listener
- `desktop/ui/src/App.tsx:1132-1198` — I5 pubkey-mismatch 模态
- `desktop/ui/src/components/SettingsPanel.tsx:60-72` — I7 autoRefresh/logFilter state
- `desktop/ui/src/components/SettingsPanel.tsx:277-328` — I7 日志面板增强 UI
- `desktop/ui/src/components/SettingsPanel.tsx:332-355` — I6 使用帮助 section

## 关联文档

- 总览：[00-release-overview.md](../../NewFunctions/00-release-overview.md)
- P2 详情：[03-p2-future-optimizations.md](../../NewFunctions/03-p2-future-optimizations.md)
- 实现规格：`docs/First/11-implementation-spec.md`
- 关联 P0 A5（公钥阻断基础）：本次 I5 在其之上增加 UI 弹窗
- 关联 D1（sender 重连）：本次 I4 修复同 key 重连路径的 latency_state 残留
