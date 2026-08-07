# 变更日志

本文件记录 SoundLink 的重要变更。格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [未发布]

### 新增

- **SoundLink Pro（open-core）工程落地**：按 `docs/NewFunctions/monetization/` 方案完成免费版与 Pro 版双构建改造。
  - **仓库切分（阶段 Q）**：新增 `desktop/pro-api`（`soundlink-pro-api`，仅 trait 与类型，MIT）与 `desktop/pro`（`soundlink-pro` 免费实现，MIT）；`soundlink` 以恒定 path 依赖二者，业务代码只按 `ProCapabilities` 能力值行事、无 `if is_pro`。私有同名 `soundlink-pro`（官方实现，闭源）检出覆盖 `desktop/pro/` 即得官方构建，构建命令与免费版完全相同；junction 下 `cargo test` / `clippy` / NSIS 打包均已实测通过。
  - **授权底座（阶段 R）**：`license/` 模块实现离线 Ed25519 签名许可证（`SLPRO-…`）验签、设备指纹（单向 SHA256 短码）、吊销名单与跨版本兼容（公钥数组化、格式版本上界、指纹候选集、SKU 白名单、新字段宽松默认）。授权存 OS keyring + `license.key` 兜底；校验失败一律降级免费版，绝不阻止启动或中断音频。新增 `get_license_status` / `activate_license` / `deactivate_license` 命令与设置页「授权」区块（指纹一键复制、激活/反激活、即时生效无需重启）。38 个 license 单测 + Python 签发 ↔ Rust 验签跨语言一致性 fixture 全绿。
  - **Pro 功能（阶段 S）**：设备记忆上限（免费 1/Pro 8，超限替换最旧并提示）；开机自启 + 启动自动收/发的判定下沉 Rust（`resolve_startup_plan`，免费实现恒 None，篡改配置无法绕过）；静默启动（自启拉起时窗口不弹出）；`last_peer_device_id` 持久化与跨启动自动重连（指数退避 1s→30s）；配置档（保存/应用/删除/重命名，`apply_profile` 复用既有命令逻辑）；全局快捷键与托盘直控能力驱动（免费仅「显示主窗口」）。
  - **签发工具链（阶段 T）**：`scripts/license/`（纯 Python Ed25519 `ed25519_pure.py`、`keygen.py` 生成 vendor 密钥对、`issue.py` 签发、`roundtrip_check.py` 跨语言一致性检查，已纳入公开 CI）。vendor 私钥经 `keygen.py` 生成并保存在仓库外，绝不入库。
  - **CI 双流水线（Q5）**：公开 CI 跑免费实现 + roundtrip 检查（无 secret，fork 可全绿）；发布 CI 检出私有实现 + `cargo clean -p soundlink-pro` 后构建官方产物。
  - ⚠ **归属调整**（`0.1.0-beta.1` 无真实用户，非回收）：`auto_start` / `auto_receive_on_start` / `auto_send_on_start` 三项自动化开关移入 Pro（免费下设置置灰、写入被忽略返回当前值）；免费版可记忆的已配对设备上限为 1 台（Pro 8 台）。音质、延迟、码率、Jitter 档位与加密全部不设限，永久免费。
- 商业化规划文档 `docs/NewFunctions/monetization/`：核心音频流转永久免费开源（MIT）+ 使用体验增强「SoundLink Pro」￥9.99 一次买断的收费边界、定价与渠道、风险清单（`00-monetization-overview.md`），以及 open-core 仓库切分与授权门控的完整工程改造方案（`01-engineering-plan.md`，阶段 Q/R/S/T/U）。授权采用离线 Ed25519 签名许可证 + 设备指纹绑定，不联网、不上报任何信息。⚠ 规划中的归属调整：现有 `auto_start` / `auto_receive_on_start` / `auto_send_on_start` 三项自启自动收发开关将在首个含 Pro 的版本移入 Pro；免费版可记忆的已配对设备上限为 1 台（Pro 8 台）。音质、延迟、码率、Jitter 档位与加密全部不设限，永久免费。
- 商业化方案补充「跨版本兼容」约束（`01-engineering-plan.md` §4.2）：明确**一次买断的 license 在所有后续版本中永久有效**——校验不比对软件版本、license 不含版本号、存储于用户配置域（keyring + `%APPDATA%\soundlink`）故覆盖安装与 NSIS 升级均不影响。同时固化 5 条只能放宽不能收紧的兼容约束（验签公钥数组化、格式版本上界判定、设备指纹算法候选集并行、SKU 白名单只增不减、新字段宽松默认）与升级保持演练项。
- 多仓库构建与使用指南 `docs/NewFunctions/monetization/02-multi-repo-guide.md`：说明 open-core 下三个 crate（公开 `soundlink-pro-api` / 公开免费实现 `soundlink-pro` / 私有同名 Pro 实现）的拓扑与依赖方向，免费版与官方版的编译命令、本地并行开发方式、CI 双流水线、`Cargo.lock` 处理、终端用户使用路径与排查表。⚠ 构建方式定稿调整：**不采用 `--features pro` + 可选私有依赖**（Cargo 会解析可选依赖，将导致无权限者的默认构建失败），改为**替换 `desktop/pro/` 目录内容**切换免费/官方构建，两者命令完全相同；官方发布线只提供一种产物，未激活时行为等同免费版。该方案已于 2026-08-06 通过临时探针工程实测确认（cargo 1.96.1），并新增三条构建硬约束：**两份 `soundlink-pro` 的 `version` 必须一致**（否则官方构建无法 `--locked`）、**每次替换 `desktop/pro/` 后必须 `cargo clean -p soundlink-pro`**（否则 Cargo 增量缓存静默复用上次实现，会构建出错版本产物且无任何报错）、**私有实现对 `soundlink-pro-api` 的 path 依赖必须用相对路径 `../pro-api`**（绝对路径会因 Windows 短名触发 lockfile collision）。
- 音频参数与自适应规划文档 `docs/NewFunctions/audio-adaptation/`：参数生效矩阵审计、码率自适应闭环（阶段 N）、真实 UDP 探测（阶段 O）、参数动态化（阶段 P）的完成计划与回填规则。
- **码率自适应闭环（阶段 N）**：接收端按丢包率计算的 `recommended_bitrate` 现在能真正改变发送端实际编码码率，无需重启流。桌面/移动发送循环内检测目标码率变化并经 Opus `set_bitrate` 热下发，带 5s 最短间隔 + 归档到允许集合的节流；`jitter_mode=auto` 时建议值自动生效，手动模式仅展示。桌面 UI 发送端面板新增「建议码率」展示与一键采纳按钮。
- **真实探测能力（阶段 O）**：桌面自动探测在样本不足（收包 < 50）时诚实返回「保持当前参数」，不再乐观误推 160kbps；`audio.params.probe_request` 实装——接收端基于真实 UDP 音频面统计回传 `probe_result`（`recommended_bitrate`/`jitter_mode`/`loss_rate`/`jitter_ms`）。移动端自动探测改走 `probe_request`/`probe_result`（替换原 5 次 TCP connect 测延迟的做法），且不再强制停止当前广播；双端探测阈值统一为 `loss_rate`/`jitter_ms` 口径。新增 AudioPacket `flags bit1=probe` 探测包标记（接收端回显且不进 Jitter Buffer/不污染统计）。
- **参数动态化（阶段 P）**：声道（Mono/Stereo）与帧长（10/20ms）端到端可变。引入运行时 `AudioFormat` 会话参数贯穿发送/接收链路；发送端采集始终 48kHz/Stereo 基线、编码前经线性插值重采样 + 声道映射转换为会话格式（新增 `format_convert` 模块），接收端按 `stream_start` 携带的会话格式重建 Opus 解码器并将解码结果重采样回 48kHz/Stereo 设备基线输出。`restart_required` 成为真实判定。桌面 UI 声道/帧长恢复可选。端到端自测（`examples/phase_p_format.rs`）验证 48k/Mono/20ms 收发零丢失。

### 变更

- **文档修正：junction 本地双开发切换降级为备选**。真实工程复现（02 文档 §11 V-8）：junction 挂载私有实现后 `cargo clean -p soundlink-pro` 失效（报 `Removed 0 files`），即便日志打印 `Compiling soundlink-pro`，产物仍可能是免费实现（社区版，无 Pro）。`docs/user/09-open-core-build.md` 与 `02-multi-repo-guide.md` 的本地切换一节改为**方式 A 物理替换（推荐）** + 方式 B junction（备选，标注缓存陷阱与验证方法），排查表与红线 G10 同步更新。
- **「开机自启动」调整为免费功能**：`auto_start` 不再属 Pro 能力，所有用户可在设置页开启/关闭（同步 autostart 注册项）。仅「自启动后自动开启接收/发送」（`auto_receive_on_start` / `auto_send_on_start`）保留为 Pro 能力（免费下置灰 + Pro 徽标）。对应 `ProCapabilities` 拆分：`autostart_available()` 恒 `true`，`automation_available()` 仅指自动收发。
- **版本号由 `0.1.0-beta.1` 调整为 `0.1.0`**（已跑 `scripts/sync_version.py` 同步 4 个清单并 `--check` 自验）：`bundle.targets:"all"` 含 MSI 目标，MSI 要求预发布标识为纯数字（≤65535），含字母的 `-beta.1` 会让 `tauri build` 在 MSI 打包阶段失败（`optional pre-release identifier ... must be numeric-only`）。去掉预发布后 MSI + NSIS 双产物均可正常打包。
- **桌面端主界面精简：音频参数与状态迁入设置页**。输出设备、Jitter 模式、音量、音频参数（采样率/声道/帧长/码率）统一移至设置页新增「音频」分区（`AudioSettingsPanel`）；主界面接收/发送模式只保留配对码（或采集源/地址/配对码）、开始/停止按钮与 3 项关键状态（连接状态、估算延迟/目标、接收/发送码率）。完整统计（丢包/缓冲/抖动/漂移/PLC 等）不再常驻主界面。⚠ 用户动作：习惯在主界面调音量/码率/Jitter 的用户，请改到「设置 → 音频」操作。
- **移动端导航精简：删除「广播」Tab，引导并入设备页**。原「广播」页仅提供分平台开启广播步骤引导，无实际连接能力（连接/配对均在设备页），已删除该 Tab；引导说明精简后并入设备页配对区块上方（仅未广播时显示）。⚠ 用户动作：底部导航由「设备/广播/设置」变为「设备/设置」。
- **移动端设备页广播中自动隐藏连接相关内容**。开始广播后，扫描/设备列表/手动 IP/配对码输入/连接按钮自动隐藏，仅保留「正在广播到 X」状态卡与「停止广播」按钮；停止后完整连接界面自动恢复。
- **音频参数采样率收窄为固定 48kHz**：spec §3.9 原声明 `sample_rate=44100|48000`，但 libopus 仅支持 8/12/16/24/48kHz，44100 会导致 `opus_encoder_create` 返回 `OPUS_BAD_ARG`，物理上不可用。已同步收窄 `11-implementation-spec.md` §3.9 与双端白名单/常量，消除文档-实现不一致。动态化维度保留声道与帧长。
- `AudioPacket` 头部 `flags` 字段文档由 u16 更正为 u8（与实现一致），并补充 bit1=probe 定义（`04-protocol.md`、`11-implementation-spec.md`）。

## [0.1.0-beta.1] - 2026-08-04

首个公开内测版本（Pre-release）。仅实测通过 Android → Windows 与 Windows → Windows；macOS 采集未实装，iOS 待真机验收。产物未经代码签名，Windows 会触发 SmartScreen 提示。

### 新增

- 开源发布配套：`CONTRIBUTING.md`、`SECURITY.md`、`CODE_OF_CONDUCT.md`、`CHANGELOG.md`、Issue / PR 模板、GitHub Actions CI 工作流。
- 英文 README（`README.en.md`）。
- 开源发布待办规划文档 `docs/NewFunctions/opensource-launch/`：总览（OSL 阶段 J/K/L/M）与市场调研（竞品对比、差异化定位、推广渠道）。
- Release 工作流 `.github/workflows/release.yml`：`v*` tag 触发，构建 Windows 免安装 exe / NSIS 安装包与 Android APK，生成 SHA256 校验文件并创建 Draft Release。
- 版本管理体系：仓库根 `VERSION` 作为版本号单一来源，`scripts/sync_version.py` 同步至 `Cargo.toml` / `tauri.conf.json` / `desktop/ui/package.json` / `pubspec.yaml`；CI 增加 `version-check` 一致性门，Release 增加 `version-gate`（校验 tag 与 `VERSION` 相等）。
- 版本管理规划文档 `docs/NewFunctions/version-management/`：架构与实现计划（V1–V15）、版本号语义与递增判定规则（含大小版本区分、`0.x` 阶段规则、AI 协作代理的版本维护义务）。

### 变更

- **改版本号方式变更**：不再手改各端清单，统一编辑根 `VERSION` 后执行 `python scripts/sync_version.py`；手改清单会被 CI 一致性门拦截。
- 版本号统一为 `0.1.0-beta.1`：移动端 `pubspec.yaml` 此前为 Flutter 模板默认值 `1.0.0+1`，与桌面端 `0.1.0` 相差一个主版本，易被误认为已发布正式版，现已对齐。
- Android APK 的 `versionCode` 改由 CI `run_number` 注入，保证单调递增（此前固定为 1，多个 APK 无法覆盖安装）。
- `AGENTS.md` / `.trae/rules/project-rules.md` 增加版本维护约束：CHANGELOG 回填义务、禁止代理自行修改 `VERSION`、破坏性变更必须醒目标注。
- README 重写：补充问题陈述、终端用户使用步骤、已知限制、贡献方向；功能矩阵区分「实测可用 / 代码就绪未实测 / 未实装」。
- `mobile/README.md`、`mobile/ios/README.md`、`mobile/android/README.md` 去除过期的「占位骨架」描述，明确 `mobile/flutter_app` 为唯一构建入口。
- 发布就绪度总览结论更新为「具备 Windows Early Access 条件」，阶段 D 标记完成。

### 修复

- 修复 Rust 1.96 新增 clippy lint 导致 CI `Clippy (tauri_app)` 步骤失败：`Default::default()` 后字段赋值改为结构体更新语法、`&PathBuf` 参数改 `&Path`、`Option::map(identity)` 冗余调用移除；3 处协议/热路径函数与 Tauri command 的参数数量告警显式 `allow` 并注明原因。
- 仓库清理：移除冗余的 `.gitignore 2`，取消跟踪 `desktop/ui/tsconfig.tsbuildinfo` 构建缓存，根目录调试文档归档至 `docs/AI_Memory/Debug/`。

---

## 0.1.0-beta.1 首发包含的研发里程碑

以下能力在首个 Release（`v0.1.0-beta.1`）之前已在 `main` 上实现，随本版一并发布。

### 桌面端（Tauri 2 + Rust + React）

- 接收端全链路：mDNS 广播 → 配对码 → UDP 接收 → 解密 → 重排 / JitterBuffer → Opus 解码 → 时钟校正 → 设备输出。
- 发送端（Windows WASAPI Loopback）：系统音频采集 → Opus 编码 → 加密 → UDP 发送，支持 backoff 断线重连。
- 系统托盘、设置面板、开机自启动、单实例锁定、窗口状态记忆、全局快捷键、首次使用引导、关于页。
- 安全：CSP 收紧、OS keyring 存私钥与固定配对码、MITM 防护、配对码错误锁定。
- 打包：NSIS 安装包（简体中文 / 英文），macOS dmg 配置就绪。

### 移动端

- Flutter 主 App：设备发现、配对、状态展示、设置、广播引导。
- Android：MediaProjection + AudioPlaybackCapture 采集，libopus JNI 编码，前台 Service 与通知。
- iOS：ReplayKit Broadcast Extension 采集 / 编码 / 发送源码就绪（待真机验收）。

### 共享层

- `shared/`：协议消息、常量、错误码单源定义。
- 加密栈：ChaCha20-Poly1305 / X25519 / Ed25519 / HKDF-SHA256 / HMAC-SHA256。

### 已实测通过的端到端组合

- Android → Windows（2026-08-02）
- Windows → Windows（2026-08-02）

---

## 回填规则

1. 每次有用户可感知的变更，写入 `[未发布]` 对应小节（新增 / 变更 / 修复 / 移除 / 安全）。
2. 发版时把 `[未发布]` 改为 `[x.y.z] - YYYY-MM-DD`，并在其上新建空的 `[未发布]`。
3. 内部重构、纯文档微调可不记录；影响构建方式或使用方式的必须记录。
