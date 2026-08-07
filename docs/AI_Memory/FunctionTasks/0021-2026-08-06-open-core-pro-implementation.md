<!-- FT-0021 -->
# SoundLink Pro（open-core）双构建落地实录（2026-08-06）

> 场景：按 `docs/NewFunctions/monetization/01-engineering-plan.md`（MON-01）完整实现免费版与 Pro 版双构建。
> 决策与收费边界见 [FT-0020](./0020-2026-08-06-monetization-plan.md)（规划），本档记录**工程实现**。

## 背景

MON-01 方案已定稿（阶段 Q/R/S/T/U）。本次把方案落成代码：open-core 三 crate 拓扑、离线 Ed25519 授权底座、PRO-1~PRO-5 功能、签发工具链与 CI 双流水线。目标：公开仓库 `cargo build` 产出完整可用免费版；私有实现检出覆盖 `desktop/pro/` 即得官方 Pro-capable 版，命令不变。

## 实现清单

| 模块 | 内容 | 关键文件 |
|---|---|---|
| 阶段 Q 仓库切分 | `soundlink-pro-api`（仅 trait/类型）+ `soundlink-pro` 免费实现；`soundlink` 恒定 path 依赖；`AppState` 挂 `caps`/`entitlement` | [desktop/pro-api/src/lib.rs](../../../desktop/pro-api/src/lib.rs)、[desktop/pro/src/lib.rs](../../../desktop/pro/src/lib.rs)、[commands/mod.rs](../../../desktop/src-tauri/src/commands/mod.rs) |
| 阶段 R 授权底座 | 离线 Ed25519 验签（base32 自实现）、设备指纹、吊销名单、跨版本兼容（公钥数组/版本上界/指纹候选/SKU 白名单/宽松默认）；license 三命令 + 设置页授权区块 | [license/mod.rs](../../../desktop/src-tauri/src/license/mod.rs)、[license/token.rs](../../../desktop/src-tauri/src/license/token.rs)、[license/fingerprint.rs](../../../desktop/src-tauri/src/license/fingerprint.rs)、[LicensePanel.tsx](../../../desktop/ui/src/components/LicensePanel.tsx) |
| 阶段 S Pro 功能 | 设备记忆上限（免费1/Pro8 替换最旧+提示）；自动化下沉 Rust（`resolve_startup_plan`，免费恒 None）；静默启动；`last_peer_device_id` + 跨启动自动重连（1s→30s 退避）；配置档五命令；快捷键/托盘能力驱动 | [trust_store.rs](../../../desktop/src-tauri/src/pairing/trust_store.rs)、[sender.rs](../../../desktop/src-tauri/src/sender.rs)、[control_server.rs](../../../desktop/src-tauri/src/network/control_server.rs)、[commands/tray.rs](../../../desktop/src-tauri/src/commands/tray.rs)、[main.rs](../../../desktop/src-tauri/src/main.rs)、[ProfilePanel.tsx](../../../desktop/ui/src/components/ProfilePanel.tsx) |
| 阶段 T 工具链 | 纯 Python Ed25519 + keygen/issue/roundtrip；vendor 密钥对生成于仓库外；roundtrip 纳入公开 CI | [scripts/license/](../../../scripts/license/) |
| 阶段 Q5 CI 双流水线 | 公开 CI（免费实现 + roundtrip，无 secret）；发布 CI（检出私有实现 + `cargo clean -p soundlink-pro`） | [ci.yml](../../../.github/workflows/ci.yml)、[release.yml](../../../.github/workflows/release.yml) |
| 私有 Pro 实现 | 仓库外 `D:\CodeProject\TRAE_Projects\soundlink-pro`（junction 挂载验证） | 仓库外（物理隔离，G3） |

## 关键设计决策

1. **能力参数化，无 `is_pro()`**：所有 Pro 差异表达为 `ProCapabilities` 返回值（设备上限/启动计划/重连策略/配置档/快捷键/托盘项）。免费实现返回真实合理降级值（记 1 台、不自动启动），不是空占位（E3）。业务代码只按能力值行事（E4/E5）。
2. **授权共享句柄即时生效**：`EntitlementHandle = Arc<RwLock<Entitlement>>` 由 AppState 与 Pro 实现共享；激活/反激活写句柄即切换能力，无需重启。
3. **签名对原始字节而非重序列化 JSON**：Python 端 canonical JSON 仅在签发时构造一次；Rust 验签直接对 base32 解码后的原始 payload 字节验签，天然规避跨语言 canonical 差异（E8 友好）。
4. **silent 只是修饰不是动作**：`StartupPlan::is_empty()` 仅看 `auto_receive`/`auto_send_to`；仅 silent 而无动作返回 None（隐藏窗口却没自动化是负体验）。此契约修正落在 pro-api。
5. **静默启动运行时判定**：`tauri.conf.json` 保持 `visible:true`，`--autostarted` 拉起且 plan.silent 时 `hide()`，免费版行为不变。

## 排障实录

- **纯 Python Ed25519 自验失败**：初版 `_self_test` 用了我凭记忆写错的 RFC 8032 期望值。用 ed25519-dalek 交叉验证同一 sk 推导的 pk，两者完全一致（`0532def3…c115`）——证实两个独立实现互证正确，错在我记忆的向量配对错配。以 dalek 为权威回拍测试向量。
- **junction 下测试复现旧行为**：改完私有实现后 `cargo test -p soundlink-pro` 仍失败——是 junction 增量缓存未失效（V-4 的又一实例），`cargo clean -p soundlink-pro` 后正常。最终定位为 `is_empty()` 语义问题（见决策 4）。
- **既有 clippy 阻塞**：`audio/opus_codec.rs` 的 `libopus_impl` 子模块测试引用了文件顶层 `frame_pcm_len`（`use super::*` 只到子模块），HEAD 即存在。补 `use crate::audio::opus_codec::frame_pcm_len;` 使 `clippy --all-targets -D warnings` 全绿（U8 硬门）。
- **MSI 打包拒绝预发布号**：`targets:"all"` 时 MSI 目标不支持 `0.1.0-beta.1`（既有配置限制，与 Pro 无关）。发布 CI 走 NSIS（`--bundles nsis` 实测产出 setup.exe 成功）。
- **junction 删除受沙箱限制**：切回免费实现时 `desktop/pro`（junction）的删除/重命名在沙箱下被拦截（`Remove-Item -Recurse` 有穿透风险，禁用）。最终需用户在终端手动 `rmdir` junction + rename 回免费实现。

## 验证结果

- `cargo test`（主 crate，tauri_app）：**112 passed**
- `cargo test -p soundlink-pro`（私有实现）：**11 passed**；`-p soundlink-pro-api`：**4 passed**
- license 模块：**38 passed**（含 Python fixture ↔ Rust 验签闭环、公钥轮换、升级保持、篡改/过期/吊销/指纹不符）
- `cargo clippy --features tauri_app --all-targets -- -D warnings`：**全绿**（junction/Pro 挂载态，V-7 ✅）
- `tauri build --bundles nsis`（junction/Pro 挂载态）：**成功产出 setup.exe**（V-6 ✅）
- `npm run build`（前端）：**通过**
- 官方构建 `--locked`：两 crate 同版本号下通过

## 用户需自行完成部分

1. ~~junction 切回免费实现~~ ✅ **已完成（2026-08-06）**：`cmd /c rmdir desktop\pro` 删 junction 本体 → `Rename-Item -Path desktop\pro-free-backup -NewName pro`（注意：位置参数 `Rename-Item A B` 对带横杠名报 PSArgumentException，须用 `-Path/-NewName`）→ `cargo clean -p soundlink-pro`。免费构建验证：`cargo test --features tauri_app` **150 测全绿**、`clippy -D warnings` 全绿。
2. **私有仓库 git 化与远端**：`D:\CodeProject\TRAE_Projects\soundlink-pro` 当前仅本地文件，需 `git init` + 推到私有远端（GitHub private repo `KversKv/soundlink-pro`），并在发布 CI 配 `PRO_REPO_TOKEN`（只读细粒度 PAT / deploy key）。
3. **vendor 私钥离线备份**：`D:\CodeProject\TRAE_Projects\soundlink-license\vendor_sk.hex` 已生成于仓库外，请离线妥善备份（丢失 = 无法签发新 key，已发出的仍永久有效）。
4. **提交**：切回免费实现后，把 `desktop/pro/`（免费版）+ `desktop/pro-api/` 一并 `git add` 提交（本次会话未做任何 commit）。
5. **T5/T6/T7**：爱发电/淘宝上架、官网 Pro 页（需外部账号与 website 目录，不在本次代码范围）。
6. **U3–U7**：免费版完整性回归、Pro 端到端、设备记忆边界、降级路径、升级保持、性能门（需真机人工演练）。

## 已知边界

- 免费实现恒返回受限值，**自动化三项开关在免费下置灰且写入被忽略返回当前值**（不报错、不写入、不动 autostart 注册项）。
- 会话内断线重连（`start_with_reconnect`）对所有用户开放——那是流转本体鲁棒性，不收费；Pro 卖的是**跨启动**自动重连。
- license 明文 `license.key` 兜底是有意为之（授权凭据非安全密钥，泄露只影响作者收入），代码注释已写明。

## 版本级别建议

**MINOR**（00 文档 §11 已定：`0.x` 阶段新增功能面 + 设备记忆上限变化按 MINOR）。CHANGELOG `[未发布]` 已回填并带 ⚠ 归属调整说明（三项自动化开关移入 Pro、免费设备记忆上限 1 台）。`VERSION` 未动（发版属产品决策）。

## 关键文件索引

- 方案：`docs/NewFunctions/monetization/01-engineering-plan.md`（Q/R/S/T/U 勾选回填）
- 构建指南：`02-multi-repo-guide.md`（V-6/V-7 勾选）
- 里程碑：`00-monetization-overview.md` §11（M-A~M-D ✅）

## 关联文档

- [FT-0020](./0020-2026-08-06-monetization-plan.md)：商业化规划与收费边界决策。
