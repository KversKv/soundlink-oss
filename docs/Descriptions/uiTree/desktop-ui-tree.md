# 桌面端 UI 控件树与交互逻辑

> 范围：本文描述当前桌面端 Tauri + React UI 的页面结构、所有可见控件、状态展示与交互逻辑。实现来源为 `desktop/ui/src/App.tsx`，桥接命令来源为 `desktop/src-tauri/src/commands/mod.rs`。

## 1. 页面总览

桌面端当前是单页应用，页面最大宽度约 560px，居中显示。顶部固定展示应用标题与副标题，下方通过角色切换按钮在「接收模式」与「发送模式」之间切换。

```text
SoundLink 桌面端
├─ 顶部标题区
│  ├─ 标题：SoundLink
│  └─ 副标题：局域网音频流转
├─ 角色切换区
│  ├─ 接收模式按钮
│  └─ 发送模式按钮
├─ 接收模式面板（role = receiver）
│  ├─ 配对码区
│  ├─ 输出设备区
│  ├─ Jitter 模式区
│  ├─ 音量区
│  ├─ 接收启停区
│  └─ 接收状态区（仅 status 存在时显示）
├─ 发送模式面板（role = sender）
│  ├─ 采集源区
│  ├─ 发现 Receiver 区
│  ├─ Receiver 地址区
│  ├─ 配对码输入区
│  ├─ 发送启停区
│  └─ 发送端状态区（仅 senderStatus 存在时显示）
├─ 全局错误提示区（仅 error 非空时显示）
└─ 底部阶段提示
```

## 2. 初始化与后台状态

### 2.1 首次加载

页面挂载后会并行初始化桌面端运行所需数据：

| 数据 | Tauri 命令 | 写入状态 | 失败处理 |
|---|---|---|---|
| 输出设备列表 | `list_output_devices` | `devices` | 将错误写入 `error` |
| 采集源列表 | `list_capture_sources` | `captureSources`、`selectedSource` | 静默忽略 |
| 当前角色 | `get_role` | `role` | 静默忽略 |
| 当前 Jitter 模式 | `get_jitter_mode` | `jitterMode` | 静默忽略 |
| 当前输出音量 | `get_volume` | `volume`，转换为 0~100 整数 | 静默忽略 |

采集源初始化时，若返回列表中存在第一个 `available = true` 的源，会自动设为当前选中采集源。

### 2.2 状态轮询

| 条件 | 周期 | Tauri 命令 | 结果 |
|---|---:|---|---|
| `running = true` | 500ms | `get_status` | 更新接收端状态 `status` |
| `senderRunning = true` | 500ms | `get_sender_status` | 更新发送端状态 `senderStatus` |

当对应运行状态变为 `false` 时，轮询定时器会被清理。

## 3. 顶部标题区

### 3.1 标题

- 控件类型：文本标题。
- 文案：`SoundLink`。
- 交互：无。

### 3.2 副标题

- 控件类型：说明文本。
- 文案：`局域网音频流转`。
- 交互：无。

## 4. 角色切换区

### 4.1 接收模式按钮

- 控件类型：按钮。
- 文案：`接收模式`。
- 激活条件：`role === "receiver"`。
- 激活样式：蓝色边框、浅蓝背景、较粗字重。
- 点击逻辑：调用 `switchRole("receiver")`。
- 后端命令：`set_role`，参数 `{ role: "receiver" }`。
- 前端状态：立即将 `role` 设置为 `receiver`，然后异步同步到 Rust Core。
- 异常处理：`set_role` 失败时静默忽略，不回滚前端角色。

### 4.2 发送模式按钮

- 控件类型：按钮。
- 文案：`发送模式`。
- 激活条件：`role === "sender"`。
- 激活样式：蓝色边框、浅蓝背景、较粗字重。
- 点击逻辑：调用 `switchRole("sender")`。
- 后端命令：`set_role`，参数 `{ role: "sender" }`。
- 前端状态：立即将 `role` 设置为 `sender`，然后异步同步到 Rust Core。
- 异常处理：`set_role` 失败时静默忽略，不回滚前端角色。

## 5. 接收模式面板

接收模式仅在 `role === "receiver"` 时渲染。此模式用于把当前电脑作为 Receiver，等待移动端或另一台电脑连接并播放音频。

### 5.1 配对码区

#### 5.1.1 区块标题

- 控件类型：标题文本。
- 文案：`配对码`。
- 交互：无。

#### 5.1.2 配对码显示框

- 控件类型：`code` 文本块。
- 显示内容：
  - 当 `pairingCode` 非空时，显示当前 8 位配对码。
  - 当 `pairingCode` 为空时，显示占位符 `— — — — — — — —`。
- 视觉特征：大字号、字符间距加大、浅灰背景。
- 交互：无。
- 安全约束：配对码只在 UI 中展示，不应落日志或持久化为明文。

#### 5.1.3 刷新按钮

- 控件类型：按钮。
- 文案：`刷新`。
- 启用条件：`running = true`。
- 禁用条件：`running = false`。
- 点击逻辑：调用 `refreshCode()`。
- 后端命令：`get_pairing_code`。
- 成功结果：更新 `pairingCode`。
- 失败结果：将错误文本写入 `error`。
- 业务含义：刷新 Receiver 当前配对码，供 Sender 输入后完成配对。

#### 5.1.4 设备 ID 文本

- 控件类型：辅助文本。
- 显示条件：`deviceId` 非空。
- 文案格式：`设备 ID：{deviceId}`。
- 交互：无。
- 来源：启动 Receiver 成功后由 `start_receiver` 返回。

### 5.2 输出设备区

#### 5.2.1 区块标题

- 控件类型：标题文本。
- 文案：`输出设备`。
- 交互：无。

#### 5.2.2 输出设备下拉框

- 控件类型：`select`。
- 当前值：`selectedDevice ?? ""`。
- 默认选项：`默认设备`，值为空字符串。
- 动态选项：来自 `devices`，每项显示 `OutputDevice.name`，选项值为设备在列表中的索引。
- 变更逻辑：调用 `pickDevice(Number(e.target.value))`。
- 后端命令：`select_output_device`，参数 `{ index }`。
- 成功结果：前端先更新 `selectedDevice`，后端记录选择。
- 失败结果：将错误文本写入 `error`。
- 生效时机：若接收引擎已经运行，后端当前逻辑提示该选择会在下一个流生效。
- 注意事项：当前默认选项值为空字符串，但变更事件会转换为 `Number(value)`；如果用户主动选回默认项，会得到 `0`，因此当前 UI 更适合首次保持默认，不适合作为显式恢复默认设备的控件。

### 5.3 Jitter 模式区

#### 5.3.1 区块标题

- 控件类型：标题文本。
- 文案：`Jitter 模式`。
- 交互：无。

#### 5.3.2 Jitter 模式按钮组

- 控件类型：一组按钮。
- 数据来源：`JITTER_MODES` 常量。
- 布局：横向排列，空间不足时自动换行。

| 值 | 显示文案 | 描述 | 业务含义 |
|---|---|---|---|
| `low` | `低延迟` | `40ms` | 更低缓冲，优先降低延迟 |
| `balanced` | `平衡` | `80ms` | 默认体验，延迟与稳定性折中 |
| `stable` | `稳定` | `150ms` | 更高缓冲，优先抗抖动 |
| `auto` | `自适应` | `动态` | 根据网络抖动动态调整 |

每个按钮的行为：

- 激活条件：`jitterMode === 当前按钮 value`。
- 激活样式：绿色边框、浅绿背景。
- 非激活样式：灰色边框、白色背景。
- 悬浮提示：按钮 `title` 为对应描述值。
- 点击逻辑：调用 `pickJitterMode(mode)`。
- 后端命令：`set_jitter_mode`，参数 `{ mode }`。
- 成功结果：前端先更新 `jitterMode`，后端同步 Jitter Buffer 模式。
- 失败结果：将错误文本写入 `error`。

### 5.4 音量区

#### 5.4.1 区块标题

- 控件类型：标题文本。
- 文案：`音量`。
- 交互：无。

#### 5.4.2 音量滑块

- 控件类型：`input[type="range"]`。
- 最小值：`0`。
- 最大值：`100`。
- 当前值：`volume`。
- 变更逻辑：调用 `changeVolume(Number(e.target.value))`。
- 后端命令：`set_volume`，参数 `{ volume: v / 100 }`。
- 成功结果：前端先更新 `volume`，后端设置接收端输出增益。
- 失败结果：将错误文本写入 `error`。
- 业务含义：控制桌面端播放输出音量，前端用百分比，后端用 0.0~1.0 浮点值。

#### 5.4.3 音量百分比文本

- 控件类型：数值文本。
- 文案格式：`{volume}%`。
- 视觉特征：右对齐、等宽数字。
- 交互：无。

### 5.5 接收启停区

#### 5.5.1 开始/停止接收按钮

- 控件类型：主按钮。
- 文案：
  - `running = false`：`开始接收`。
  - `running = true`：`停止接收`。
- 颜色：
  - `running = false`：绿色。
  - `running = true`：红色。
- 点击逻辑：
  - `running = false` 时调用 `start()`。
  - `running = true` 时调用 `stop()`。

开始接收流程：

1. 清空 `error`。
2. 调用后端命令 `start_receiver`。
3. 成功后写入：
   - `pairingCode = r.pairing_code`
   - `deviceId = r.device_id`
   - `running = true`
4. 失败时将错误文本写入 `error`。
5. `running = true` 后启动接收状态轮询。

停止接收流程：

1. 清空 `error`。
2. 调用后端命令 `stop_receiver`。
3. 成功后写入：
   - `running = false`
   - `status = null`
4. 失败时将错误文本写入 `error`。
5. `running = false` 后停止接收状态轮询。

### 5.6 接收状态区

接收状态区仅在 `status` 非空时显示。状态来源为 `get_status` 轮询结果。

#### 5.6.1 区块标题

- 控件类型：标题文本。
- 文案：`状态`。
- 交互：无。

#### 5.6.2 状态定义列表

- 控件类型：`dl` 定义列表。
- 布局：两列网格，左侧字段名，右侧字段值。

| UI 字段 | 数据来源 | 展示格式 | 含义 |
|---|---|---|---|
| 状态 | `status.state` | 原样显示 | Receiver 状态机当前状态 |
| 已收包 | `status.packets_recv` | 整数 | 已接收 UDP 音频包数量 |
| 丢包 | `status.packets_lost`、`lossPct` | `{packets_lost}（{lossPct}%）` | 丢失包数量与丢包率 |
| 丢弃 | `status.packets_dropped` | 整数 | 过期或无效包丢弃数量 |
| 缓冲 | `status.buffer_ms`、`status.buffer_depth` | `{buffer_ms} ms（{buffer_depth} 帧）` | 当前 Jitter Buffer 深度 |
| 抖动 | `status.jitter_ms` | `{jitter_ms} ms` | 估算网络抖动 |
| 估算延迟 | `status.est_latency_ms` | `{est_latency_ms} ms` | 端到端延迟估算 |
| 接收码率 | `bitrateKbps` | `{bitrateKbps} kbps` | 当前接收码率 |
| 建议码率 | `recBitrateKbps` | `{recBitrateKbps} kbps`，必要时追加 `（自适应）` | Receiver 根据弱网情况推荐给 Sender 的码率 |
| 漂移校正 | `driftPct` | `{driftPct}%` | 输出时钟漂移修正比例 |
| 连续 PLC | `status.consecutive_plc` | `{n} 帧` | 连续补偿帧数 |
| Jitter 模式 | `status.jitter_mode` | 原样显示 | 后端实际 Jitter 模式 |

派生值规则：

- `lossPct = (status.loss_rate * 100).toFixed(1)`。
- `bitrateKbps = Math.round(status.bitrate / 1000)`。
- `recBitrateKbps = Math.round(status.recommended_bitrate / 1000)`。
- `driftPct = ((status.drift_ratio - 1) * 100).toFixed(2)`。
- 当 `recBitrateKbps > 0 && recBitrateKbps !== 128` 时，在建议码率后显示 `（自适应）`。

## 6. 发送模式面板

发送模式仅在 `role === "sender"` 时渲染。此模式用于把当前电脑作为 Sender，将本机测试音源或系统音频发送到局域网中的 Receiver。

### 6.1 采集源区

#### 6.1.1 区块标题

- 控件类型：标题文本。
- 文案：`采集源`。
- 交互：无。

#### 6.1.2 采集源下拉框

- 控件类型：`select`。
- 当前值：`selectedSource`。
- 数据来源：`captureSources`。
- 选项文案：
  - 可用源：`{name}`。
  - 不可用源：`{name}（不可用）`。
- 禁用规则：当 `CaptureSourceInfo.available = false` 时，对应 `option` 禁用。
- 变更逻辑：直接更新 `selectedSource`。
- 后端同步：不立即调用后端；开始发送时作为 `captureSource` 参数传入 `start_sender`。
- 当前典型源：
  - `sine`：`440Hz 正弦测试源`，始终可用。
  - Windows + `wasapi` feature：`WASAPI Loopback（系统音频）`，可用。
  - macOS：`ScreenCaptureKit（未实现）`，不可用。

### 6.2 发现 Receiver 区

#### 6.2.1 区块标题

- 控件类型：标题文本。
- 文案：`发现 Receiver`。
- 交互：无。

#### 6.2.2 扫描局域网按钮

- 控件类型：按钮。
- 文案：
  - `discovering = false`：`扫描局域网`。
  - `discovering = true`：`扫描中...`。
- 禁用条件：`discovering = true` 或 `senderRunning = true`。
- 点击逻辑：调用 `discoverReceivers()`。
- 后端命令：`discover_receivers`，参数 `{ durationSecs: 3 }`。

扫描流程：

1. 清空 `error`。
2. 设置 `discovering = true`。
3. 调用 `discover_receivers` 扫描 mDNS Receiver。
4. 成功后将返回列表写入 `discovered`。
5. 若发现至少一个 Receiver 且 `receiverAddr` 为空，自动把第一个设备的 `control_addr` 填入 `receiverAddr`。
6. 失败时将错误文本写入 `error`。
7. 无论成功失败，最终设置 `discovering = false`。

#### 6.2.3 Receiver 发现结果列表

- 控件类型：无序列表。
- 显示条件：`discovered.length > 0`。
- 每个列表项代表一个 `DiscoveredReceiver`。
- 主文本：`device_name`。
- 辅助文本：`control_addr` + 配对状态。
- 配对状态文案：
  - `pairing_required = true`：`· 需配对`。
  - `pairing_required = false`：`· 已信任`。
- 点击逻辑：将该设备的 `control_addr` 写入 `receiverAddr`。
- 后端同步：不立即连接；开始发送时使用该地址。

#### 6.2.4 未发现设备提示

- 控件类型：辅助文本。
- 显示条件：`discovered.length === 0 && !discovering`。
- 文案：`未发现设备，可手动输入地址。`
- 交互：无。

### 6.3 Receiver 地址区

#### 6.3.1 区块标题

- 控件类型：标题文本。
- 文案：`Receiver 地址`。
- 交互：无。

#### 6.3.2 Receiver 地址输入框

- 控件类型：文本输入框。
- 当前值：`receiverAddr`。
- 占位符：`192.168.1.100:47810`。
- 禁用条件：`senderRunning = true`。
- 变更逻辑：直接更新 `receiverAddr`。
- 校验逻辑：开始发送时只校验非空，不在输入阶段校验 IP、端口或格式。
- 业务含义：控制通道地址，通常来自 mDNS 发现结果，也可手动输入。

### 6.4 配对码输入区

#### 6.4.1 区块标题

- 控件类型：标题文本。
- 文案：`配对码`。
- 交互：无。

#### 6.4.2 Sender 配对码输入框

- 控件类型：文本输入框。
- 当前值：`senderPairingCode`。
- 占位符：`8 位配对码（已信任设备可留空）`。
- 禁用条件：`senderRunning = true`。
- 变更逻辑：直接更新 `senderPairingCode`。
- 后端同步：不立即调用后端；开始发送时作为 `pairingCode` 参数传入 `start_sender`。
- 安全约束：配对码不应写入日志；已信任 Receiver 可留空，由后端信任路径处理。

### 6.5 发送启停区

#### 6.5.1 开始/停止发送按钮

- 控件类型：主按钮。
- 文案：
  - `senderRunning = false`：`开始发送`。
  - `senderRunning = true`：`停止发送`。
- 颜色：
  - `senderRunning = false`：蓝色。
  - `senderRunning = true`：红色。
- 点击逻辑：
  - `senderRunning = false` 时调用 `startSender()`。
  - `senderRunning = true` 时调用 `stopSender()`。

开始发送流程：

1. 清空 `error`。
2. 若 `receiverAddr` 为空，设置 `error = "请输入或选择 Receiver 地址"` 并停止流程。
3. 调用后端命令 `start_sender`，参数：
   - `receiverAddr`
   - `pairingCode: senderPairingCode`
   - `captureSource: selectedSource`
4. 成功后设置 `senderRunning = true`。
5. 失败时将错误文本写入 `error`。
6. `senderRunning = true` 后启动发送端状态轮询。

停止发送流程：

1. 清空 `error`。
2. 调用后端命令 `stop_sender`。
3. 成功后写入：
   - `senderRunning = false`
   - `senderStatus = null`
4. 失败时将错误文本写入 `error`。
5. `senderRunning = false` 后停止发送端状态轮询。

### 6.6 发送端状态区

发送端状态区仅在 `senderStatus` 非空时显示。状态来源为 `get_sender_status` 轮询结果。

#### 6.6.1 区块标题

- 控件类型：标题文本。
- 文案：`发送端状态`。
- 交互：无。

#### 6.6.2 发送端状态定义列表

- 控件类型：`dl` 定义列表。
- 布局：两列网格，左侧字段名，右侧字段值。

| UI 字段 | 数据来源 | 展示格式 | 含义 |
|---|---|---|---|
| 状态 | `senderStatus.state` | 原样显示 | Sender 状态机当前状态 |
| 目标 | `senderStatus.receiver_device_name || senderStatus.target_addr` | 优先设备名，否则地址 | 当前连接目标 |
| 已发包 | `senderStatus.packets_sent` | 整数 | 已发送 UDP 音频包数量 |
| 编码耗时 | `senderStatus.encode_ms_avg` | `{value.toFixed(1)} ms` | Opus 编码平均耗时 |
| 发送码率 | `senderBitrateKbps` | `{senderBitrateKbps} kbps` | 当前发送码率 |
| 已信任 | `senderStatus.trusted` | `是` / `否` | 是否走已信任设备路径 |
| 错误 | `senderStatus.error` | 红色错误文本 | 后端 Sender 错误信息，仅非空时显示 |

派生值规则：

- `senderBitrateKbps = Math.round(senderStatus.bitrate / 1000)`。
- 目标显示优先级为设备名高于地址。

## 7. 全局错误提示区

- 控件类型：错误文本。
- 显示条件：`error` 非空。
- 文案格式：`错误：{error}`。
- 颜色：红色。
- 来源：各交互函数的 `catch`，以及发送前地址为空的前端校验。
- 清理时机：
  - 开始/停止接收前清空。
  - 扫描 Receiver 前清空。
  - 开始/停止发送前清空。
  - 部分设置类操作失败会写入错误，但成功时不主动清空旧错误。

## 8. 底部阶段提示

- 控件类型：辅助文本。
- 文案：`阶段 5：桌面发送端（双电脑互传）。运行 cargo run --example phase5_loopback 自测。`
- 交互：无。
- 业务含义：提示当前桌面 UI 已进入阶段 5 的双电脑互传自测范围。

## 9. 前端状态模型

### 9.1 接收端状态

| 状态名 | 类型 | 初始值 | 用途 |
|---|---|---|---|
| `running` | `boolean` | `false` | Receiver 是否已启动 |
| `pairingCode` | `string` | `""` | Receiver 当前配对码 |
| `deviceId` | `string` | `""` | 当前桌面设备 ID |
| `devices` | `OutputDevice[]` | `[]` | 输出设备列表 |
| `selectedDevice` | `number \| null` | `null` | 选中的输出设备索引 |
| `status` | `ReceiverStatus \| null` | `null` | Receiver 运行状态快照 |
| `jitterMode` | `JitterMode` | `balanced` | 当前 Jitter 模式 |
| `volume` | `number` | `100` | 输出音量百分比 |

### 9.2 发送端状态

| 状态名 | 类型 | 初始值 | 用途 |
|---|---|---|---|
| `senderRunning` | `boolean` | `false` | Sender 是否已启动 |
| `senderStatus` | `SenderStatus \| null` | `null` | Sender 运行状态快照 |
| `receiverAddr` | `string` | `""` | 目标 Receiver 控制地址 |
| `senderPairingCode` | `string` | `""` | Sender 输入的配对码 |
| `discovered` | `DiscoveredReceiver[]` | `[]` | mDNS 发现到的 Receiver 列表 |
| `captureSources` | `CaptureSourceInfo[]` | `[]` | 可选采集源列表 |
| `selectedSource` | `string` | `sine` | 当前采集源 ID |
| `discovering` | `boolean` | `false` | 是否正在扫描局域网 |

### 9.3 全局状态

| 状态名 | 类型 | 初始值 | 用途 |
|---|---|---|---|
| `role` | `receiver \| sender` | `receiver` | 当前 UI 模式 |
| `error` | `string` | `""` | 全局错误提示 |

## 10. 后端命令映射

| UI 行为 | 命令 | 方向 | 备注 |
|---|---|---|---|
| 初始化输出设备 | `list_output_devices` | UI → Rust | 返回输出设备列表 |
| 初始化采集源 | `list_capture_sources` | UI → Rust | 返回 sine / WASAPI / ScreenCaptureKit 等源 |
| 初始化角色 | `get_role` | UI → Rust | 返回 `receiver` 或 `sender` |
| 切换角色 | `set_role` | UI → Rust | 只记录 UI 角色，不自动启停流 |
| 初始化 Jitter 模式 | `get_jitter_mode` | UI → Rust | 返回后端当前模式 |
| 设置 Jitter 模式 | `set_jitter_mode` | UI → Rust | 运行时影响 Jitter Buffer |
| 初始化音量 | `get_volume` | UI → Rust | 后端 0.0~1.0，前端转百分比 |
| 设置音量 | `set_volume` | UI → Rust | 前端百分比转 0.0~1.0 |
| 启动接收 | `start_receiver` | UI → Rust | 生成配对码，启动 mDNS、控制服务与接收能力 |
| 停止接收 | `stop_receiver` | UI → Rust | 停止控制服务、mDNS 与 UDP 接收 |
| 刷新配对码 | `get_pairing_code` | UI → Rust | 重新签发配对码 |
| 选择输出设备 | `select_output_device` | UI → Rust | 参数为设备索引 |
| 轮询接收状态 | `get_status` | UI → Rust | 500ms 周期，仅接收运行时 |
| 扫描 Receiver | `discover_receivers` | UI → Rust | 当前扫描 3 秒 |
| 启动发送 | `start_sender` | UI → Rust | 连接 Receiver，握手，采集并发送音频 |
| 停止发送 | `stop_sender` | UI → Rust | 停止 Sender 引擎 |
| 轮询发送状态 | `get_sender_status` | UI → Rust | 500ms 周期，仅发送运行时 |

## 11. 交互边界与当前限制

- 角色切换不会自动停止当前 Receiver 或 Sender；如果在运行中切换角色，后台任务仍依赖各自启停按钮控制。
- Receiver 地址输入只做非空校验，不校验地址格式、端口范围或连接可达性。
- 配对码输入不限制 8 位数字格式，格式错误由后端配对流程返回错误。
- 采集源选择在 Sender 运行期间没有禁用，但地址与配对码输入会禁用；实际采集源变更只在下一次 `start_sender` 生效。
- 输出设备下拉框包含默认设备选项，但当前变更处理会将空字符串转换为数字，显式切回默认设备的语义需要后续补强。
- 已信任设备管理命令已存在于后端，但当前桌面 UI 尚未提供「信任设备列表 / 移除信任」控件。
- 设备显示名设置命令已存在于后端，但当前桌面 UI 尚未提供「修改设备名」控件。

## 12. 推荐的后续 UI 完善方向

| 方向 | 建议控件 | 目的 |
|---|---|---|
| 信任设备管理 | 已信任设备列表、移除按钮 | 让用户可撤销配对信任 |
| 设备名设置 | 文本输入框、保存按钮 | 影响 mDNS 广播展示名 |
| 运行中保护 | 角色切换前提示或自动停流 | 避免前台模式与后台流状态不一致 |
| 输入校验 | 地址格式校验、配对码数字校验 | 提前给出可理解错误 |
| 默认设备恢复 | 明确的「系统默认设备」选项语义 | 避免空值转数字导致歧义 |
| 连接可视化 | 状态徽标、连接阶段进度 | 降低配对/握手/流启动过程的不确定感 |
