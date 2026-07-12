<!-- NF-02 -->
# P1 · Beta 发布前重要补强

> 优先级：🟠 P1 · 目标版本：v0.2.0 前
> 范围：重连 / 单实例 / 退出清理 / 关于页 / 窗口记忆 / 引导 / LICENSE / 隐私政策 / 文档同步

---

## 阶段 D · 健壮性补强

**目标**：网络断开/异常退出/多实例场景下应用行为可控。

### 进度表

- [ ] D1 · 网络断开自动重连 — `desktop/src-tauri/src/sender.rs:715-783`、`receiver.rs:392-396`
  - 当前：sender 检测到断开后任务直接退出，UI 需手动重连；receiver UDP recv 错误时 `break`
  - 目标：
    - sender：控制循环退出后启动 backoff 重试（5s/10s/30s 三档），UI 提示「重连中…」
    - receiver：UDP recv 错误时记录日志并继续，不直接 break
  - 验证：主动断网 → 重连；网络恢复后自动恢复收发
- [ ] D2 · 单实例锁定 — `desktop/src-tauri/Cargo.toml`
  - 当前：未启用 `tauri-plugin-single-instance`，双击图标启动多实例导致 UDP 端口冲突（`DEFAULT_AUDIO_PORT=47811`）
  - 目标：加 `tauri-plugin-single-instance = "2"` 依赖；`main.rs` setup 中初始化；二次启动时聚焦既有窗口
  - 验证：运行中再双击图标，不启新实例，既有窗口聚焦
- [ ] D3 · 退出时优雅停止收发 — `desktop/src-tauri/src/commands/tray.rs` quit 路径
  - 当前：`app.exit(0)` 未先调 `stop_receiver`/`stop_sender`，依赖 `Drop`；采集线程 `join()` 可能阻塞 1s+ 导致退出卡顿
  - 目标：quit 前显式调用 `stop_receiver`/`stop_sender`，等待任务退出后再 `app.exit`
  - 验证：发送中点退出，1s 内进程消失，无端口残留
- [ ] D4 · 配对失败重试引导 — `desktop/ui/src/App.tsx:423`、`desktop/src-tauri/src/pairing_code.rs:128-150`
  - 当前：5 次错误后锁定，UI 无「等待解锁」提示
  - 目标：前端展示剩余尝试次数；锁定后倒计时提示「请 X 秒后重试」
  - 验证：故意输错配对码，UI 显示剩余次数与锁定倒计时
- [ ] D5 · DeviceIdentity 加载失败处理 — `desktop/src-tauri/src/commands/mod.rs:69-77`
  - 当前：加载失败仅 warn 后用临时身份，重启后身份变化导致已信任设备失效
  - 目标：临时身份场景下 UI 强提示「设备身份损坏，请重新配对所有设备」；或尝试备份恢复
  - 验证：手动破坏 identity 文件，重启后 UI 弹明确警告

**阶段验收**：
- [ ] 网络断开 30s 内自动恢复
- [ ] 双击图标不启动多实例
- [ ] 退出时 1s 内进程消失
- [ ] 配对失败有重试引导

---

## 阶段 E · 用户体验补全

**目标**：补齐「关于」页、窗口记忆、首次引导、设置面板扩展。

### 进度表

- [ ] E1 · 关于页面 + 版本号显示 — `desktop/ui/src/components/SettingsPanel.tsx`
  - 当前：仅「启动」「关闭窗口行为」两节，UI 无版本号
  - 目标：新增「关于」节，显示版本/构建时间/作者/许可证/仓库地址
  - 验证：设置页底部可见版本号与项目信息
- [ ] E2 · 窗口大小/位置记忆 — `desktop/src-tauri/Cargo.toml`
  - 当前：每次启动固定 510×780，无 `tauri-plugin-window-state`
  - 目标：加 `tauri-plugin-window-state = "2"` 依赖；`main.rs` setup 中初始化
  - 验证：调整窗口大小并退出，重启后保持上次大小
- [ ] E3 · 首次使用引导 — `desktop/ui/src/components/`
  - 当前：无 onboarding
  - 目标：新建 `Onboarding.tsx`，3 步引导（选角色 → 选设备 → 测试连接）；首次启动显示，后续不再弹
  - 验证：删除配置后重启，弹出引导；完成后再启动不弹
- [ ] E4 · 设置面板补齐 — `desktop/ui/src/components/SettingsPanel.tsx`
  - 当前：仅 4 个开关 + 1 组单选
  - 目标：补充：设备名修改、默认采集源选择、日志查看（只读）
  - 验证：设置页可改设备名且立即生效
- [ ] E5 · 加载状态指示 — `desktop/ui/src/App.tsx`
  - 当前：`start_receiver`/`start_sender` 按钮点击后无反馈，`discoverReceivers` 有 `discovering` 布尔
  - 目标：所有长任务按钮加 `busy` 状态（spinner + 禁用）
  - 验证：点击「开始接收」后按钮显示 spinner 直到返回
- [ ] E6 · 空状态处理 — `desktop/ui/src/App.tsx`
  - 当前：接收端首次启动无空态引导，status 为 null 时状态卡无显示
  - 目标：未启动时显示「点击上方按钮开始接收」提示
  - 验证：首次启动 UI 无空白区域

**阶段验收**：
- [ ] 设置页有「关于」节并显示版本号
- [ ] 窗口大小/位置跨重启保持
- [ ] 首次启动有引导
- [ ] 所有长任务有 loading 反馈

---

## 阶段 F · 合规与文档

**目标**：补齐 LICENSE、隐私政策、README、AGENTS 状态同步。

### 进度表

- [ ] F1 · 添加 LICENSE 文件 — 仓库根
  - 当前：全仓无 LICENSE；`Cargo.toml` 无 `license` 字段
  - 目标：决定许可证（MIT / Apache-2.0 / AGPL-3.0），落到仓库根；`Cargo.toml` 加 `license` 字段
  - 验证：`cargo publish --dry-run` 不报缺 license
- [ ] F2 · 隐私政策 — `docs/privacy.md` 或 `docs/legal/privacy.md`
  - 当前：无；项目涉及采集系统音频（WASAPI Loopback）
  - 目标：说明「采集系统音频仅在用户主动开启发送模式时进行，不离开局域网」；列出收集的本地数据（配置/信任设备列表/日志）；声明不上报任何遥测
  - 验证：UI 设置页「关于」节可链接到隐私政策
- [ ] F3 · README 完整化 — `README.md`、`desktop/README.md`
  - 当前：根 README 仍写「仓库为骨架 + 占位说明，尚未进行脚手架初始化」与实际矛盾；`desktop/README.md` 仅 12 行
  - 目标：根 README 更新为项目介绍 + 截图 + 快速开始 + 文档导航；`desktop/README.md` 更新桌面端构建/运行说明
  - 验证：新用户按 README 可在 30 分钟内跑通
- [ ] F4 · AGENTS.md 状态同步 — `AGENTS.md:46-47`
  - 当前：「仓库为骨架 + 占位... 尚未进行脚手架初始化」与 `12-plan.md` 阶段 1/3/4 已完成矛盾
  - 目标：更新「当前状态」段为「阶段 5 进行中；Windows 桌面端可用，macOS 采集未实装」
  - 验证：AI 协作代理读 AGENTS.md 不会被误导
- [ ] F5 · 用户使用文档 — `docs/user/desktop-guide.md`
  - 当前：`docs/user/` 仅有开发环境文档，无终端用户文档
  - 目标：写一份面向终端用户的桌面端使用指南（安装、配对、收发、常见问题）
  - 验证：非技术用户可按文档独立完成配对收发
- [ ] F6 · DRM 受保护内容提示 — `desktop/ui/src/App.tsx` 发送模式
  - 当前：无提示
  - 目标：发送模式首次开启时提示「部分受 DRM 保护的应用音频可能无法采集，属系统限制」
  - 验证：首次点「开始发送」弹出提示

**阶段验收**：
- [ ] 仓库根有 LICENSE
- [ ] 有隐私政策文档
- [ ] README 与实际进度一致
- [ ] AGENTS.md 状态同步

---

## 关联文档

- 总览：[00-release-overview.md](./00-release-overview.md)
- P0 红线：[01-p0-blocking-fixes.md](./01-p0-blocking-fixes.md)
- P2 优化：[03-p2-future-optimizations.md](./03-p2-future-optimizations.md)
