<!-- FT-0017 -->
# 首版 v0.1.0-beta.1 初始化实录（2026-08-04）

> 场景：用户提交版本管理体系后，要求把当前状态定为「第一版正式内测版本」并完成首次版本初始化。
> 关联：[FT-0016](./0016-2026-08-04-version-management-v1-v5.md)（版本管理体系落地，本次的前置）

## 1. 需求与决策

用户原话：「我进行了提交，现在作为第一版正式的内测版本；帮我进行第一次版本的初始化」。经确认两项决策：

| 决策点 | 结论 | 理由 |
|---|---|---|
| 版本号 | 保持 `0.1.0-beta.1`，**不 bump** | 内测 = 预发布，`-beta.1` 后缀准确传达「不保证稳定」，`release.yml` 会据此自动标记 Pre-release |
| 执行范围 | 改文档 + commit + 打**本地** tag，**不 push** | 用户明确授权，覆盖项目规则「禁 git commit」红线；push 与 Release 发布仍归人类 |

## 2. 执行前核对

- 工作区干净，`605b2c7 Add Version Manage` 已提交
- 仓库无任何既有 tag（本次为首个）
- `VERSION` = `0.1.0-beta.1`，`sync_version.py --check` EXIT=0

## 3. 撞上的阻塞：CI clippy 门必红

按 OSL 发布前必查清单跑第一项时发现：

```
cargo clippy --features tauri_app --all-targets -- -D warnings
→ exit 101（lib 10 errors + lib test 12 errors）
```

该命令即 [`ci.yml` 第 80 行](file:///d:/CodeProject/TRAE_Projects/SoundLink/oss/.github/workflows/ci.yml#L79-L80) 的原文。**若直接打 tag 并 push，CI 立即失败，初始化目的（产出可用 Release）即未达成**，故暂停 commit/tag，先修 lint。

根因：本地 toolchain 为 **rust 1.96.1 / clippy 0.1.96**，新增或加强的 lint 命中存量代码；不是本次改动引入的回归（`cargo test` 在修复前就是 63 passed 全绿）。

## 4. 修复清单

用户选择「先修完 clippy 再发版」。先 `cargo clippy --fix` 自动修 3 处，余下手工处理。

| 文件 | lint | 处理 |
|---|---|---|
| [`audio/capture/wasapi_loopback.rs`](file:///d:/CodeProject/TRAE_Projects/SoundLink/oss/desktop/src-tauri/src/audio/capture/wasapi_loopback.rs) | `map_identity` | `--fix` 自动移除冗余 `.map()` |
| [`device/device_identity.rs`](file:///d:/CodeProject/TRAE_Projects/SoundLink/oss/desktop/src-tauri/src/device/device_identity.rs) | `io_other_error` ×2 | `--fix` 改为 `std::io::Error::other(_)` |
| [`commands/mod.rs`](file:///d:/CodeProject/TRAE_Projects/SoundLink/oss/desktop/src-tauri/src/commands/mod.rs) | `derivable_impls` | `--fix` 改用 `#[derive(Default)]` |
| [`config/mod.rs`](file:///d:/CodeProject/TRAE_Projects/SoundLink/oss/desktop/src-tauri/src/config/mod.rs#L120-L142) | `field_reassign_with_default` ×2 | 手工改结构体更新语法 `Self { fixed_pairing_code: .., ..Self::default() }`，语义等价 |
| [`config/mod.rs`](file:///d:/CodeProject/TRAE_Projects/SoundLink/oss/desktop/src-tauri/src/config/mod.rs#L428-L438) | 同上（测试内） | `save_then_load_roundtrip` 同样改法 |
| [`logging/mod.rs`](file:///d:/CodeProject/TRAE_Projects/SoundLink/oss/desktop/src-tauri/src/logging/mod.rs#L88) | `ptr_arg` | `&PathBuf` → `&Path`，并在 `daily_writer` 内补 `use std::path::Path` |
| [`sender.rs:92`](file:///d:/CodeProject/TRAE_Projects/SoundLink/oss/desktop/src-tauri/src/sender.rs#L91-L93) | `type_complexity` | `on_state_change` 加 `#[allow]`（同文件 `on_pubkey_mismatch` 早已如此处理，保持一致） |
| [`sender.rs:461`](file:///d:/CodeProject/TRAE_Projects/SoundLink/oss/desktop/src-tauri/src/sender.rs#L460-L464) `handshake` | `too_many_arguments` 8/7 | `#[allow]` + 注明「握手参数均为协议必需字段」 |
| [`sender.rs:716`](file:///d:/CodeProject/TRAE_Projects/SoundLink/oss/desktop/src-tauri/src/sender.rs#L717-L721) `send_loop` | 同上 | `#[allow]` + 注明「音频热路径句柄，避免额外包装」 |
| [`commands/mod.rs:967`](file:///d:/CodeProject/TRAE_Projects/SoundLink/oss/desktop/src-tauri/src/commands/mod.rs#L964-L968) `set_app_settings` | 同上 | `#[allow]` + 注明「Tauri command 参数需与前端字段一一对应」 |

### 关键决策：为何 4 处用 `#[allow]` 而非重构

三处 `too_many_arguments` 均只超 1 个（8/7），拆参数需改函数签名，触及握手协议与音频热路径 —— 发首版前动核心链路有回归风险，且违反「最小改动」原则。`type_complexity` 同理。均加注释说明原因，非无脑压制。

## 5. 验证结果

| 项 | 结果 |
|---|---|
| `cargo clippy --all-targets -- -D warnings` | EXIT=0 |
| `cargo clippy --features tauri_app --all-targets -- -D warnings` | EXIT=0 |
| `cargo test` / `--features opus` / `--features wasapi` | 63 / 64 / 76 passed，0 failed |
| `npm run build`（desktop/ui） | built in 615ms |
| `flutter analyze` / `flutter test` | No issues found / 8 passed |
| `sync_version.py --check` | EXIT=0（VERSION 0.1.0-beta.1，build_number 100） |

## 6. 交付物

- `CHANGELOG.md`：`[未发布]` → `[0.1.0-beta.1] - 2026-08-04` 并在其上留空 `[未发布]`；补首发免责摘要（仅实测 Android→Windows / Windows→Windows、macOS 未实装、产物未签名会触发 SmartScreen）；里程碑小节改名并修正过期 tag 名 `v0.1.0-beta` → `v0.1.0-beta.1`；按义务 A 追加 clippy 修复条目
- [`00-launch-overview.md`](file:///d:/CodeProject/TRAE_Projects/SoundLink/oss/docs/NewFunctions/opensource-launch/00-launch-overview.md#L50-L88)：K3 标 `[~]`（本地 tag 已打，push 待人工）；发布前必查清单 6/7 勾选并记录核验结论
- [`12-plan.md`](file:///d:/CodeProject/TRAE_Projects/SoundLink/oss/docs/First/12-plan.md#L28)：发布侧进度改为首版已本地定版
- commit `c41dd04`（8 files changed，46+/31-）
- 本地 annotated tag `v0.1.0-beta.1` → `c41dd04`

## 7. 建议版本级别

**不升版本**。本次是把既有 `0.1.0-beta.1` 定版发布，非版本号变更。clippy 修复属内部质量改动，无对外行为变化，已按义务 A 记入 CHANGELOG 该版本的「修复」小节。

## 8. 用户需自行完成

1. `git push origin main`（如需）与 `git push origin v0.1.0-beta.1` —— push tag 后 `release.yml` 触发
2. 等 Actions 完成，到 Releases 页核对 Draft 产物与 SHA256
3. **用 Draft 里的 Release 产物（非本地 dev）重跑一次 Android → Windows 端到端** —— 清单唯一未闭环项
4. 按 K4 补 Release Notes 三条免责后 Publish 为 Pre-release

## 9. 已知边界

- 本地 clippy 1.96.1 与 CI `dtolnay/rust-toolchain@stable` 可能存在版本差异，CI 仍有出现新 lint 的可能
- `cargo fmt --check` 在 CI 中仍为 `continue-on-error: true`（待 OSL-L1 全量格式化后移除），本次未触碰
