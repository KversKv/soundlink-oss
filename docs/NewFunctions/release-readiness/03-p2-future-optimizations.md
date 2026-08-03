<!-- NF-03 -->
# P2 · 后续版本优化

> 优先级：🟡 P2 · 目标版本：v1.0.0 前
> 范围：跨平台补全 / 测试覆盖 / 体验优化 / 长时稳定性

---

## 阶段 G · 跨平台补全

**目标**：兑现 macOS/Linux 跨平台承诺。

### 进度表

- [ ] G1 · macOS ScreenCaptureKit 采集实现 — `desktop/src-tauri/src/audio/capture/macos_screencapturekit.rs:33`
  - 当前：`"ScreenCaptureKit 采集尚未实现（需 macOS 环境 + SCStream API）"` 占位；`commands/mod.rs:498-505` 在 macOS 暴露的该源标 `available:false`
  - 目标：实装 SCStream 采集系统音频 → PCM（48kHz/Stereo/Int16）→ 接入既有 Opus 编码链路
  - 依赖：macOS 13+、ScreenCaptureKit framework、`objc2-screen-capture-kit` 或裸 FFI
  - 验证：macOS 播放音乐，Windows 端能听到
- [ ] G2 · Linux PipeWire 输出实现 — `desktop/src-tauri/src/audio/output/linux_pipewire.rs:1`
  - 当前：5 行注释占位
  - 目标：实装 PipeWire 输出，或退而用 cpal 的 ALSA 后端（先验证 cpal 在 Linux 是否可用）
  - 验证：Linux 桌面能播放接收到的音频
- [ ] G3 · macOS 输出验证 — `desktop/src-tauri/src/audio/output/macos_coreaudio.rs:1`
  - 当前：注释占位
  - 目标：验证 cpal 在 macOS 下 CoreAudio 输出可用（理论已支持，需实测）
  - 验证：macOS 接收模式可正常播放
- [x] G4 · Linux WASAPI feature no-op 验证 — `desktop/src-tauri/Cargo.toml:24` — 2026-07-12 注释微调 + Windows 端 `cargo check --no-default-features --features tauri_app` 通过；macOS/Linux 真机构建待 G1/G2/G3
  - 当前：注释「非 Windows 平台 wasapi feature 为 no-op」
  - 目标：CI 在 Linux/macOS 构建验证 `tauri_app` feature 不报错
  - 验证：跨平台 CI 通过
- [x] G5 · macOS 安装包 dmg 配置 — `desktop/src-tauri/tauri.conf.json` — 2026-07-12 tauri.conf.json 补 macOS 节点（signingIdentity/entitlements/minimumSystemVersion/dmg）；dmg 实际生成待 macOS 真机
  - 当前：无 `bundle.macOS` 节点
  - 目标：增加 `dmg` 配置（windowPosition、background、iconSize）
  - 验证：`cargo tauri build` 在 macOS 生成 `.dmg`

**阶段验收**：
- [ ] macOS 端到端可用（采集 + 接收）
- [ ] Linux 输出可用
- [ ] 跨平台 CI 全绿

---

## 阶段 H · 测试与稳定性

**目标**：建立自动化测试基线，覆盖回归风险。

### 进度表

- [x] H1 · `commands/mod.rs` 单元测试 — `desktop/src-tauri/src/commands/mod.rs` — 2026-07-12 14 个纯函数单测通过（parse_role/role_as_str/parse_jitter_mode/nearest_bitrate/make_capture_source）
  - 当前：0 测试
  - 目标：覆盖 `set_app_settings`/`get_app_settings`/`set_close_action`/`set_auto_start` 等纯逻辑命令（不依赖 Tauri 上下文的）
  - 验证：`cargo test --features tauri_app` 命令模块有测试通过
- [x] H2 · `tray.rs` 单元测试 — `desktop/src-tauri/src/commands/tray.rs` — 2026-07-12 抽 `decide_close_action` 纯函数 + `CloseDecision` 枚举 + 6 单测通过
  - 当前：0 测试
  - 目标：测试 `handle_close_requested` 三分支逻辑（mock state.close_action）
  - 验证：三分支单测通过
- [x] H3 · `config/mod.rs` 单元测试 — `desktop/src-tauri/src/config/mod.rs` — 2026-07-12 13 个单测覆盖 load_or_default 三分支/normalized 6 字段/save roundtrip/backup
  - 当前：0 测试
  - 目标：覆盖 `AppConfig::load_or_default`（含损坏文件）、`normalized`、新字段默认值
  - 验证：损坏文件场景返回默认且备份生成
- [x] H4 · `wasapi_loopback.rs` 单元测试 — `desktop/src-tauri/src/audio/capture/wasapi_loopback.rs` — 2026-07-12 8 个 ring buffer 边界单测通过（poll_frame 四场景 + cap 限制 + 构造/生命周期）
  - 当前：0 测试
  - 目标：mock WASAPI 接口测试环形缓冲读写边界
  - 验证：缓冲边界条件单测通过
- [ ] H5 · 端到端测试框架 — `desktop/ui/` 新增
  - 当前：无 E2E
  - 目标：接入 Playwright 或 Tauri WebDriver，覆盖：启动 → 配对 → 收发 → 退出
  - 验证：CI 跑 E2E 通过
- [x] H6 · CI 流水线 — `.github/workflows/ci.yml` — 2026-07-12 已实装
  - 现状：3 个 job — `desktop-rust`（windows/ubuntu 矩阵，clippy `-D warnings` + `--features tauri_app` + `cargo test` / `opus` / `wasapi`）、`desktop-ui`（`npm ci` + `npm run build`）、`mobile-flutter`（`dart format` 校验 + `analyze` + `test`）
  - 遗留：`cargo fmt` 步骤为 `continue-on-error`，待 OSL-L1 全量格式化后阻塞化；E2E（H5）尚未接入 CI
  - 验证：PR / push 触发 CI 全绿
- [ ] H7 · 长时压测报告 — 新增 `docs/test/`
  - 当前：无
  - 目标：1h+ 连续收发测试，记录内存/CPU/丢包率曲线
  - 验证：1h 运行内存增长 < 10MB

**阶段验收**：
- [ ] 单测覆盖率 ≥ 60%
- [ ] CI 全绿
- [ ] 1h 压测无内存泄漏

---

## 阶段 I · 体验优化

**目标**：性能、易用性、可访问性提升。

### 进度表

- [ ] I1 · 500ms 轮询改事件驱动 — `desktop/ui/src/App.tsx:264-280`、`desktop/src-tauri/src/receiver.rs`
  - 当前：前端 setInterval 500ms 调 `get_status`/`get_sender_status`，每秒 2 次跨 IPC 取整个 status 结构体
  - 目标：Rust 端状态变化时 emit `status-changed` 事件；前端 listen 替代轮询；保留 1s 心跳作兜底
  - 验证：CPU 占用下降 ≥30%
- [x] I2 · 全局快捷键 — `desktop/src-tauri/src/main.rs` — 2026-07-12 注册 tauri-plugin-global-shortcut + Ctrl+Shift+P/S 两个快捷键 + 前端 listen 通过；macOS Cmd 映射待真机
  - 当前：无
  - 目标：注册全局快捷键（如 Ctrl+Shift+P 切换角色、Ctrl+Shift+S 显示主窗口）
  - 依赖：`tauri-plugin-global-shortcut`
  - 验证：应用隐藏时按快捷键可呼出
- [ ] I3 · 多语言支持（i18n） — `desktop/ui/src/`
  - 当前：仅简体中文
  - 目标：接入 i18next，支持中英文切换；设置页加语言选择
  - 验证：切换到英文 UI 全部翻译
- [x] I4 · 重连后 latency_state 重置 — `desktop/src-tauri/src/receiver.rs` — 2026-07-12 start() 改全量重置 + 同 key 重连路径补 reset_latency_state()
  - 当前：`latency_state.bitrate_start`/`first_recv_instant` 仅在 `start()` 中重置，重连路径不重置
  - 目标：重连时显式重置 latency_state
  - 验证：断网重连后码率统计正确
- [x] I5 · 信任公钥不一致时阻断 — `desktop/src-tauri/src/sender.rs:581-596` — 2026-07-12 emit `pubkey-mismatch` 事件 + 前端模态弹窗（删除并重配对/取消）；安全语义不变仍立即 return Err
  - 当前：仅 `warn!` 不阻断（P0 A5 已处理严格化，这里进一步增加 UI 提示）
  - 目标：UI 弹窗「检测到对端身份变化，可能存在中间人攻击，是否继续？」
  - 验证：模拟公钥变化，UI 弹窗确认
- [x] I6 · 帮助文档集成 — `desktop/ui/src/components/` — 2026-07-12 SettingsPanel 加「使用帮助」section（外链文档/反馈 + 快速上手说明）
  - 当前：无应用内帮助
  - 目标：设置页加「使用帮助」按钮，跳转本地 HTML 或外链
  - 验证：点击帮助可查看
- [x] I7 · 日志查看面板 — `desktop/ui/src/components/` — 2026-07-12 日志面板加刷新按钮 + 自动刷新（5s）+ 关键字过滤
  - 当前：仅 Rust 端 tracing 输出到 stderr
  - 目标：Rust 端把最近 N 条日志缓存到内存；前端设置页加「查看日志」按钮，弹出只读面板
  - 验证：UI 可查看最近 200 条日志
- [ ] I8 · 窗口主题切换 — `desktop/ui/src/`
  - 当前：仅浅色主题
  - 目标：跟随系统主题（light/dark）；CSS 变量化
  - 验证：系统切换暗色后 UI 跟随

**阶段验收**：
- [ ] CPU 占用下降 ≥30%
- [ ] 全局快捷键可用
- [ ] 中英文切换无遗漏
- [ ] 暗色主题可用

---

## 关联文档

- 总览：[00-release-overview.md](./00-release-overview.md)
- P0 红线：[01-p0-blocking-fixes.md](./01-p0-blocking-fixes.md)
- P1 重要项：[02-p1-important-improvements.md](./02-p1-important-improvements.md)
