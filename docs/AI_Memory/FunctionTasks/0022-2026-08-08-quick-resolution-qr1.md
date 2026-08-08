<!-- FT-0022 -->

# QR-1 分辨率快速切换（Pro）全量实现（2026-08-08）

> 场景：按 `docs/NewFunctions/display/display.md` 给 Pro 版添加分辨率快速切换功能（M0–M9 全量）。

## 背景/需求

用户新增 `1920×1440 480Hz` 这类自定义分辨率后，需要付一次「提权 + 3 秒黑屏」的预置代价，
之后每次切换就是普通分辨率切换（毫秒级）。DSC 场景下 NVIDIA 驱动禁用自定义分辨率，
必须走 EDID Override 注入。本期仅 NVIDIA + Windows。

## 实现清单（M0–M9）

| 里程碑 | 交付 | 关键文件 |
|---|---|---|
| 基础设施 | `ProCapabilities::quick_resolution_available()` 能力位（免费 false/官方按授权） | `oss/desktop/pro-api/src/lib.rs`、`oss/desktop/pro/src/lib.rs`、`pro/src/lib.rs` |
| qr-bandwidth | 带宽/像素时钟/DSC 判定公式（11 测试） | `src-tauri/crates/qr-bandwidth/src/lib.rs` |
| qr-edid | EDID 解析/编辑/checksum/CVT-RB v2/v3/native-blanking（18 测试） | `src-tauri/crates/qr-edid/src/{lib,parse,edit,timing}.rs` |
| qr-ipc | 主进程 ↔ helper 共享协议（纯 serde） | `src-tauri/crates/qr-ipc/src/lib.rs` |
| M1 后端 | model/store/platform(CCD+GDI)/applier/rollback/commands + Pro 门控 + 启动自检 | `src-tauri/src/features/quick_resolution/` |
| M1 前端 | 设置区 UI、编辑弹窗+预设库+可行性预检、识别叠层、15s 确认窗、系统导入 | `ui/src/features/quickResolution/` |
| M2 托盘 | 二级菜单、多屏分组、✓ 标记、200ms 防抖、恢复上一个 | `features/quick_resolution/tray.rs` + `commands/tray.rs` |
| M3 DSC | NVAPI 动态加载 + 三路交叉判定 + 徽标 + 手动覆盖 | `platform/windows/nvapi/` + `dsc.rs` |
| M4 helper | 计划任务、命名管道+ACL+nonce+签名校验、审计日志、watchdog、restore-all | `src/bin/qr_helper.rs` + `helper_core/` + `helper_client.rs` |
| M6+M7 | 能力探测阶梯 + 批量预置编排 + 三层黑屏保险 + Stale 重预置 | `capability.rs` + `provisioner.rs` |
| M8+M9+M0 | NVAPI 自定义（占位，FFI 未验证）、热键、退出恢复、诊断导出、qr-probe | `nvapi/custom.rs` + `hotkey.rs` + `src/bin/qr_probe.rs` |

## 关键设计决策

1. **门控对齐既有 open-core 架构**：文档 §十二用 `license::gate(FEATURE_QR)`，但本仓库门控走
   `ProCapabilities` 能力值（E4/E5/G6）。落地位 `ProCapabilities::quick_resolution_available()`，
   免费实现返回 `false`、官方实现按 `is_pro()` 返回。免费版设置区遮罩、所有 `qr_*` 命令返回
   `FeatureLocked`。
2. **nonce 传递改临时文件**：文档 §4.1 用计划任务 `$(Arg0)` 注入 nonce，但 `schtasks /Run`
   不支持携带参数。改为 `%APPDATA%/soundlink/qr_nonce.tmp`（helper `--serve` 读后删除）。
3. **NVAPI FFI 保守降级**：`NvDisplayPortInfoV1`/`NvTiming` 结构体布局未逐字节对照官方头文件
   验证（公开 nvapi.h 最佳理解）。调用可能在真实 NVIDIA 机器上 UB（M0 qr_probe 实测任务）。
   因此 M8 `try_custom_display` 等保守返回 `NvApiUnavailable`，单测不调用 FFI（曾因此访问冲突）。
4. **helper Adapter 重启简化**：helper 侧 `find_display_adapter_instance()` 返回 None（适配器
   实例路径需 CCD source id 链，helper 进程无 CCD 上下文）。激活阶梯在 Monitor 重启失败后由
   主进程侧 `provisioner` 决定是否走 Adapter（当前主进程也未实现 Adapter 重启，保守降级为
   `LogoffRequired` 提示）。M6 能力探测实测补齐。
5. **托盘失败走系统通知**：用 PowerShell `NotifyIcon` balloon（零新增插件依赖），不弹主窗。
6. **识别叠层/确认窗复用主 bundle**：`WebviewUrl::App("index.html?view=qr-confirm/qr-identify")`，
   App.tsx 按 `?view=` 短路渲染，不新增窗口配置。

## 验证结果

- `cargo check --no-default-features --features tauri_app`：通过
- `cargo test --no-default-features --features tauri_app`：191 lib + 2 probe + 29 crates = 222 全绿
- `cargo clippy --no-default-features --features tauri_app --lib`：通过（0 error）
- `npm run build`（ui）：通过（tsc + vite）

## 用户需自行完成部分

1. **M0 实测**：`cargo run --bin qr_probe --features tauri_app -- --test-mode 1920x1440@480`
   在真实 NVIDIA 机器上验证：NVAPI 布局正确性、DSC 字段、注册表变体、带宽可行性。
2. **M7 实机预置**：需管理员权限装 helper（`qr_install_helper`），EDID 注入 + 设备重启
   有真实黑屏风险，请在测试机验证三层保险。
3. **M8 NVAPI 自定义**：FFI 布局实测通过后，移除 `custom.rs` 中保守的 `NvApiUnavailable` 返回。

## 已知边界

- 仅 Windows（非 Windows 后端为 stub，命令返回 `UnsupportedPlatform`）。
- 仅 NVIDIA（AMD/Intel 走 `DisplayBackend` trait 插拔，本期未实现）。
- 小数刷新率不支持（需求边界）。
- HDR/VRR 不主动改动（切换后系统重置仅提示，未实现状态校验）。
- 多显示器热插拔后 `qr://display-changed` 触发刷新，但「换口后 MonitorKey 漂移」未实测。
- helper 的 Authenticode 签名校验为「同目录」简化版（开发期未签名二进制可通过）；
  交付构建应收紧为「同目录 + 有效签名」。

## 关键文件索引

- 后端核心：`src-tauri/src/features/quick_resolution/{model,store,service,applier,rollback,provisioner,capability,hotkey,tray,commands}.rs`
- 平台层：`src-tauri/src/features/quick_resolution/platform/windows/{ccd,gdi,edid_reg,device_restart,dsc,helper_client,identify,monitor_evt}.rs` + `nvapi/{mod,ffi,custom}.rs`
- helper：`src-tauri/src/bin/qr_helper.rs` + `features/quick_resolution/helper_core/{mod,pipe_server,scheduled_task,audit,watchdog}.rs`
- 探针：`src-tauri/src/bin/qr_probe.rs`
- 纯逻辑 crates：`src-tauri/crates/qr-{bandwidth,edid,ipc}/`
- 前端：`ui/src/features/quickResolution/{QuickResolutionSection,ModeEditorDialog,FeasibilityHint,ConfirmWindow,IdentifyOverlay,api,types,presets}.tsx/ts`
- 接线：`src-tauri/src/{lib.rs,main.rs,commands/mod.rs,commands/tray.rs}`、`pro-api/src/lib.rs`、`pro/src/lib.rs`、`pro/src/lib.rs`（私有）

## 关联文档

- 设计文档：`docs/NewFunctions/display/display.md`
- open-core 门控：[FT-0021](./0021-2026-08-06-open-core-pro-implementation.md)
- 商业化方案：[FT-0020](./0020-2026-08-06-monetization-plan.md)

## 建议版本级别

**MINOR**（0.1.0 → 0.2.0）：新增完整 Pro 功能（QR-1），用户可感知、影响构建（新增两个 bin、
`zip` 依赖、capabilities 权限）。`0.x` 阶段破坏性变更降级走 MINOR，本功能无破坏性变更。
