<!-- FT-0019 -->

# 双端 UI 精简：音频参数迁设置页 + 移动端删广播 Tab（2026-08-05）

> 场景：用户提出三点 UI 诉求 —— ① 桌面端音频参数/状态迁入设置页保持主界面干净可直接用；② 移动端「广播」页是否冗余（设备页即可配对并广播），无用则删；③ 移动端设备页开始广播后应自动隐藏连接相关内容、断开后恢复。

## 背景与决策

经 `AskUserQuestion` 与用户确认三项决策：

1. 桌面端状态卡片：**精简主界面只留关键项**（连接状态、估算延迟、接收/发送码率 3 项），完整统计移入主界面外。
2. 移动端广播 Tab：**删除 Tab，引导并入设备页**（未广播时显示，广播中自动隐藏）。
3. 桌面端音频参数：**全部移入设置页**（输出设备/Jitter/音量/采样率/声道/帧长/码率），主界面两种角色共用同一份设置。

根因判断：移动端原「广播」页（`broadcast_guide_page.dart`）仅提供分平台开启广播步骤引导，无任何连接/配对能力（这些都在设备页 `discovery_page.dart`），属冗余页面，符合删除条件。

## 实现清单

### 桌面端（desktop/ui）

| 变更 | 文件 |
|---|---|
| 新增「音频」设置分区组件（输出设备/Jitter/音量/音频参数 + 自动探测），自主管理 state 直调后端命令 | [AudioSettingsPanel.tsx](file:///d:/CodeProject/TRAE_Projects/SoundLink/oss/desktop/ui/src/components/AudioSettingsPanel.tsx) |
| 设置页挂载 `<AudioSettingsPanel />`（位于「设备」与「日志」之间） | [SettingsPanel.tsx](file:///d:/CodeProject/TRAE_Projects/SoundLink/oss/desktop/ui/src/components/SettingsPanel.tsx) |
| 主界面删除：接收模式「输出设备/Jitter/音量」卡 +「音频参数」卡；发送模式「音频参数」卡；两模式状态卡精简为 3 项关键指标 | [App.tsx](file:///d:/CodeProject/TRAE_Projects/SoundLink/oss/desktop/ui/src/App.tsx) |
| 清理无引用：`OutputDevice` 接口、`JITTER_MODES`/`BITRATE_OPTIONS` 等常量、`pickDevice`/`pickJitterMode`/`changeVolume`/`autoDetectAudioParams`/`setAudioParams` 函数、`devices`/`volume`/`jitterMode` state 与相关 effect、派生值 `lossPct`/`recBitrateKbps`/`driftPct`/`senderRecKbps`/`senderAdoptKbps` | [App.tsx](file:///d:/CodeProject/TRAE_Projects/SoundLink/oss/desktop/ui/src/App.tsx) |

说明：`selectedDevice` state 保留（仍被 `Onboarding` 引用）；`audioParams` state 保留（`adaptiveOn` 用于发送码率「（自动）」角标）。

### 移动端（mobile/flutter_app）

| 变更 | 文件 |
|---|---|
| 底部导航由「设备/广播/设置」改为「设备/设置」，移除广播 Tab 与对应 `IndexedStack` 子页 | [home_page.dart](file:///d:/CodeProject/TRAE_Projects/SoundLink/oss/mobile/flutter_app/lib/src/pages/home_page.dart) |
| 设备页：未广播时在配对区块上方新增 `_buildGuideSection`（精简版 iOS/Android 开启广播步骤 + DRM 说明）；广播中 `broadcasting` 分支仅渲染 `_buildBroadcastingCard`（状态卡 + 停止按钮），隐藏扫描/设备列表/手动 IP/配对输入/连接按钮 | [discovery_page.dart](file:///d:/CodeProject/TRAE_Projects/SoundLink/oss/mobile/flutter_app/lib/src/pages/discovery_page.dart) |
| 配对区块 `_buildPairingSection` 移除原内嵌「停止广播」按钮（广播中已由独立卡片承担） | [discovery_page.dart](file:///d:/CodeProject/TRAE_Projects/SoundLink/oss/mobile/flutter_app/lib/src/pages/discovery_page.dart) |
| 删除冗余广播引导页 | `lib/src/pages/broadcast_guide_page.dart`（已删） |
| widget 测试同步移除「广播」Tab 断言 | [widget_test.dart](file:///d:/CodeProject/TRAE_Projects/SoundLink/oss/mobile/flutter_app/test/widget_test.dart) |

## 关键设计决策

- **广播中判定**：以 `app.conn == LinkState.streaming` 为唯一信号，广播中整页切换为精简布局，断开后自动回退完整连接界面（同一 `ListenableBuilder` 内条件渲染，无需额外状态）。
- **引导精简**：原 5 步 `ListTile` 改为编号文本行，压缩纵向空间，避免设备页过长；iOS 额外保留「不支持静默改全局音量」提示。
- **设置页音频分区自治**：`AudioSettingsPanel` 不复用 App.tsx 的 state，而是自建 state + 直接 `invoke`，避免主界面与设置页状态耦合；主界面不再加载输出设备/音量/Jitter。

## 验证结果

- 桌面端：`tsc -b` exit 0；`npm run build`（tsc + vite build）exit 0，39 modules transformed。
- 移动端：`flutter analyze` → `No issues found!`（exit 1 来自 sandbox 对 `.dartServer` 缓存目录的限制，与代码无关）；`flutter test` → `All tests passed!`（8/8）。
- Rust 后端未改动（`set_audio_params`/`select_output_device`/`set_volume` 等命令保持原样），无需重测。

## 用户需自行完成部分

- 实机/实窗体验证：桌面端「设置 → 音频」调参后主界面运行是否如预期；移动端广播中界面精简与停止后恢复的实际交互手感。
- iOS 真机广播引导步骤文案仍待实机验收（与阶段 2 iOS 待实机一致）。

## 已知边界

- 主界面不再展示完整统计（丢包/缓冲/抖动/漂移/PLC 等）；如需排查问题须看日志（设置页 → 日志预览）。这是「主界面干净」与「可观测性」的取舍，已由用户确认。
- 移动端广播中隐藏了「上次连接/已信任设备」列表，因此广播中无法切换目标设备，需先停止再重连（符合「广播中隐藏连接相关」的预期）。

## 回填与版本

- `CHANGELOG.md [未发布]` 已新增 3 条「变更」条目（桌面端主界面精简 / 移动端删广播 Tab / 移动端广播中隐藏连接内容），均带 ⚠ 用户动作说明。
- `docs/First/12-plan.md` 无需改动：本次为 UI 布局优化，不新增/完成阶段任务勾选。
- **建议版本级别：MINOR**。理由：`0.x` 阶段含用户可感知的导航/界面布局变更（删 Tab、参数入口迁移），属行为变化但非破坏性协议变更，按 `01-versioning-policy.md` 走 MINOR；未自行改 `VERSION`（发版属产品决策）。

## 关键文件索引

- 桌面：`desktop/ui/src/{App.tsx, components/SettingsPanel.tsx, components/AudioSettingsPanel.tsx}`
- 移动：`mobile/flutter_app/lib/src/pages/{home_page.dart, discovery_page.dart}`、`mobile/flutter_app/test/widget_test.dart`
- 文档：`CHANGELOG.md`

## 关联文档

- 前序桌面 UI 重构：[FT-0008](./0008-2026-07-07-desktop-ui-redesign.md)
- 音频参数持久化：[FT-0011](./0011-2026-07-07-persistent-settings-audio-sync.md)
- 版本维护义务：`docs/NewFunctions/version-management/01-versioning-policy.md`
