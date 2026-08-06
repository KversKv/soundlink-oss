<!-- FT-0020 -->
# 商业化方案设计：核心免费开源 + Pro 买断（2026-08-06）

> 场景：用户希望「核心功能开源免费、体验优化功能买断制（约 ￥9.99）」以获取一定利润，要求分析收费边界并在 `docs/NewFunctions/` 产出收费框架与完整工程改造方案。
> 本次会话**只产出规划文档，未改动任何代码**。

---

## 1. 需求与两次纠偏

| 轮次 | 用户要求 | 对方案的影响 |
|---|---|---|
| 初始 | 核心免费开源，体验优化功能买断 ￥9.99；在 `docs/NewFunctions/` 建文档保存收费框架 + 完整改造方案 | 确立目录 `docs/NewFunctions/monetization/` |
| 纠偏 1 | 「核心就是音频流转功能，而非其它额外的功能用来区分 Pro 和免费版；应该是例如开机自动进入发送模式这种体验上优化明显的功能」 | **推翻**第一版「Pro = EQ / 多接收端广播 / 诊断报表 / 主题包」，收窄为流转本体的体验优化 |
| 纠偏 2 | ① 不受 `0.1.0-beta.1` 影响（无真实用户，仅流程测试）② Pro 需包含 `auto_start` / `auto_receive_on_start` / `auto_send_on_start` ③ 开源方式更改：不希望直接编译即得完全版本 | **推翻**「全部 MIT + 许可证开关」，改为 open-core（私有 Pro crate）；解除「已发布功能不可追溯收费」约束 |

---

## 2. 最终决策

| 决策项 | 结论 |
|---|---|
| 开源模型 | **主仓库 MIT（免费核心完整可编译可用）+ Pro 逻辑在私有 crate `soundlink-pro`** |
| 收费范围 | 仅桌面端；移动端全功能免费 |
| 价格 | ￥9.99 一次买断，永久，含后续 Pro 新功能，个人最多 3 台设备 |
| 授权校验 | 离线 Ed25519 签名 license + 设备指纹绑定；**零联网零遥测** |
| 渠道 | 爱发电（主）+ 淘宝小店 |
| Pro 范围 | PRO-1 自启+启动即收/发；PRO-2 记忆并自动重连上次设备；PRO-3 多设备记忆（免费 1 台 / Pro 8 台）；PRO-4 多套配置一键切换；PRO-5 全局快捷键与托盘直控 |
| 价值主张 | **「插上耳机就有声音，不用打开 SoundLink。」** 免费版约 5 步 → Pro 版 0 步 |

明确排除的收费候选（避免方案再次发散）：EQ / 录制 / 诊断图表 / 主题包 / 多接收端广播 / 更低延迟档位 / 跨平台支持 / i18n / 支持优先级 / 时长次数限制。

---

## 3. 交付文件

| 文件 | 内容 |
|---|---|
| [`00-monetization-overview.md`](../../NewFunctions/monetization/00-monetization-overview.md) | 分工、决策记录（含两条被推翻结论留档）、划分原则 P1–P5、免费能力清单、Pro 清单 PRO-1~5、对照表、排除清单、open-core 形态与诚实沟通口径、定价渠道、收入预期与止损、风险清单、里程碑 M-A~M-E、回填规则 |
| [`01-engineering-plan.md`](../../NewFunctions/monetization/01-engineering-plan.md) | 工程红线 E1–E8、现状基线（逐项标代码位置）、仓库切分方案与 `ProCapabilities` trait 草案、阶段 Q（仓库切分）/ R（授权底座 + license 规格 + 跨版本兼容 §4.2）/ S（Pro 功能 S1–S15）/ T（签发与销售）/ U（测试质量门）、实施顺序、不做清单 |
| [`02-multi-repo-guide.md`](../../NewFunctions/monetization/02-multi-repo-guide.md) | 多仓库形态与 crate 拓扑、切换机制（目录替换）、免费/官方构建命令、本地并行开发（junction）、edition 自我标识、版本号与 `Cargo.lock` 规则、分发决策（只发一种产物）、终端用户路径、CI 双流水线、排查表、红线 G1–G9、落地前实测项 V-1~V-5 |

---

## 3b. 第二轮追加（同日）

用户追问两点：① 当前方案是否考虑软件更新后买断自动适配；② 补一份多仓库管理/编译/使用文档。

### 3b.1 买断跨版本自动适配（任务 1）

核查结论：**现有方案默认已具备该性质**，因为 license payload 无版本号、校验不比对软件版本、存储在用户配置域（keyring `soundlink` + `%APPDATA%\soundlink`）而非安装目录，NSIS 升级与 exe 覆盖都不触碰。

但发现两个真实隐患，已补成工程约束（`01` 文档 §4.2 C1–C5 + 任务 R8/R9/R10）：

| # | 隐患 | 约束 |
|---|---|---|
| C1 | 单个 `PUBKEY_VENDOR_B64` 常量形态，日后轮换密钥会让存量 license 全废 | **一开始就做成公钥数组** `PUBKEYS_VENDOR: &[&str]`，只增不删（R8） |
| C2 | `v == 1` 式精确匹配会让旧 key 在格式升级后失效 | 改为 `v <= LICENSE_FORMAT_MAX`（R8） |
| C3 | 指纹算法虽有 `soundlink-fp-v1` 前缀，但未设计并行校验路径 | `fingerprint_candidates() -> Vec<String>` 候选集语义，旧算法永久保留（R9） |
| C4 | `sku` 字符串重命名会废掉旧 key | 白名单常量，只增不减（R8） |
| C5 | 新增 payload 字段若默认值偏严，旧 key 会被判失效 | `#[serde(default)]` + **宽松方向**默认值 |

另新增红线 E8、测试项 U6b（NSIS 升级 + exe 覆盖两条路径的升级保持演练）、以及一条判据：「任何 license 校验相关改动，先问『已发出的 key 改动后还能通过吗』，答案非『能』则不做」。

同时确认项目**未接入 `tauri-plugin-updater`**（全项目 grep 无匹配），这对本方案有利；若将来接入须免费/Pro 一视同仁、元数据不带 license。

### 3b.2 多仓库指南（任务 2）

写作过程中**推翻了首轮的 `--features pro` + 可选私有 git 依赖方案**：

> Cargo 在解析依赖图时会连**可选依赖**一并解析并写入 `Cargo.lock`，与 feature 是否启用无关。公开仓库一旦写入私有 git 依赖，无访问权的社区用户连默认 `cargo build` 都会 403 失败 —— 直接违反红线 E3 与「fork CI 可通过」目标。

改为 **目录替换方案**：

- `desktop/src-tauri/Cargo.toml` 恒定 `path` 依赖 `../pro-api` 与 `../pro`，无 feature、无 optional。
- `desktop/pro/` 放公开免费实现 → 社区构建；私有实现检出覆盖 → 官方构建。**命令完全相同。**
- 两份 `soundlink-pro` **crate 名相同**，各自导出 `EDITION`（`community` / `official`）仅用于显示与日志，**不得用于门控**。

连带的其它决策：

| # | 决策 | 理由 |
|---|---|---|
| D9 | Pro crate **不进** `sync_version.py` 的 `TARGETS` | path 依赖不写版本约束；加进去等于把发版流程绑死两个仓库。⚠ 但两份 pro crate 的 `version` 字面值必须一致（见 D13），因其写定后不再变动，故仍无需纳入自动同步 |
| ~~D10~~ → 见 D13 | ~~Pro 构建不加 `--locked`~~ **已放宽**：版本号一致且私有实现未引入新依赖时，官方构建**也应加 `--locked`**（首选状态）；一旦引入新依赖则去掉，且 lock 变更绝不回提公开仓库 | 原判断基于「两侧版本不一致导致 lock 必变」的错误前提；统一版本号后 lock 全程稳定。不回提的红线不变（E7） |
| D11 | 官方发布线**只发一种产物**（Pro-capable，未激活等同免费），产物名不加 `-pro` 后缀 | 用户下载一次即可，粘贴 key 就解锁；也让 §4.2 的更新适配天然成立 |
| D12 | 私有仓库**物理放在公开仓库目录之外**，本地用 junction 切换 | 放内部即使有 `.gitignore`，一次 `git add -f` 就泄露 |
| **D13** | 两份 `soundlink-pro` 的 `version` 字段**必须完全相同** | `Cargo.lock` 记录 path 依赖的版本号；不一致时替换目录会触发 `Updating soundlink-pro v0.1.0 -> v2.7.3` 改写 lock，与 `--locked` 直接冲突（V-3/V-5 实测） |
| **D14** | 每次替换 `desktop/pro/` 后**必须** `cargo clean -p soundlink-pro` | crate 名/版本/path 均未变，Cargo 指纹不认为源码有变化，会静默复用旧 `.rlib`，**双向串味且无任何警告**——免费目录可能构建出 Pro 产物（泄露闭源实现），反之则 Pro 产物缺功能（V-4 实测，升级为红线 G10） |
| **D15** | 私有仓库对 `soundlink-pro-api` 的 path 依赖用**相对路径** `../pro-api` | 绝对路径会因 Windows 短名（`ADMINI~1`）与 Cargo 内部解析的长名不一致，触发 `package collision in the lockfile`（V-2 实测）。副作用：私有仓库**无法脱离公开仓库独立构建**，这是预期行为 |

因方案变更，同步回改了 `00` 文档 §7.1/§7.2 与 M-B、`01` 文档 §2.1/§2.4/Q1–Q3/Q5。

### 3b.3 构建方案实测验证（同日第三轮）

用户诉求：「验证并确认方案，之外我进行开发」。在 `%TEMP%\slverify` 建临时探针工程实测，环境 `cargo 1.96.1 / rustc 1.96.1`（Windows PowerShell 5）：

- **V-1 探针**：一个 crate，写 `optional = true` 的不可访问 GitHub 私有仓库依赖 + `default = []`。
- **V-2~V-5 探针**：复刻三 crate 拓扑（`slv-pro-api` / `slv-pro` / `slv-app`），另在工程外放同名 `slv-pro` 官方实现（`EDITION = "official"`、`max_remembered_devices() = 8`），用 junction 挂载切换。

| 项 | 结论 |
|---|---|
| V-1 | ✅ **与预期一致**。未启用的可选 git 依赖仍被解析：默认 `cargo build` 报 `Updating git repository` → `Repository not found` → 重试 3 次 → `failed to get 'ghost-pro' as a dependency`。用不存在的 `path` 依赖同样失败。→ **feature 方案彻底否决，不保留回退路径** |
| V-2 | ✅ junction 免管理员权限即可创建（`New-Item -ItemType Junction`），Cargo 完全穿透；相对路径 `../pro-api` 按挂载后位置解析到公开仓库 |
| V-3 | ⚠ 同名**不同版本**可构建，但会改写 `Cargo.lock` → 推出 D13 |
| V-4 | ❌ **增量缓存确实双向串味，本方案最大的坑**。回切免费实现后仍输出 `edition=official`（`Finished` 0.03s，完全没重编译）；反向同样。`cargo clean -p` 双向均能修正 → 推出 D14 / 红线 G10 |
| V-5 | ⚠ `--locked` 失败信息清晰（`cannot update the lock file … because --locked was passed`），但根因是版本号不一致；统一后 `--locked` 通过 → D10 放宽 |

排查过程中一度怀疑 `Remove-Item -Force` 穿透 junction 删掉了私有实现源码，经双向 `Get-ChildItem -Recurse` 与读文件核查排除，锁定为 Cargo 指纹问题。但该风险真实存在，故 `02` §3.4 已改为用 `(Get-Item desktop\pro).Delete()` 删 junction，并显式禁用 `Remove-Item -Recurse -Force`。

**未实测（低风险，留待 Q 阶段真实工程内确认）**：V-6 Tauri NSIS 完整打包在替换目录后的表现；V-7 `cargo clippy` 在 junction 下的表现。二者预期与已验证行为一致。

验证结论已回填 `02` §3.1/§3.3/§3.4/§5.1/§5.2/§8/§9/§10/§11/§12、`01` §2.1 与 Q2/Q3/Q5、`00` §7.1、`12-plan.md`、`CHANGELOG.md`。临时探针工程已删除（junction 已安全移除；`%TEMP%` 下残留 82 个被进程锁定的构建产物文件，不影响仓库）。

---

## 4. 关键设计决策

| # | 决策 | 理由 |
|---|---|---|
| D1 | trait 边界 `pro_api` 放**公开侧**，免费实现是真实降级行为而非空洞占位 | 公开仓库读起来自洽完整（「免费版就是记 1 台、不自动启动」），避免被指责开源诱饵；同时 Pro 实现不公开、编译不出来 |
| D2 | `ProCapabilities` **不设 `is_pro()`**，全部表达为能力参数（`max_remembered_devices()` / `startup_plan()` / `reconnect_policy()` …） | 门控单点（E4），业务代码只按能力值行事，不散落 `if is_pro` |
| D3 | trait 需抽到**第三个独立公开 crate** | 公开 crate 依赖私有 crate、私有 crate 又需 trait 定义，否则循环依赖 |
| D4 | 自动收发逻辑**必须从前端下沉到 Rust**（`resolve_startup_plan()`） | 现实现在 `App.tsx` mount 时驱动，前端门控可被改前端资源绕过 |
| D5 | 设备记忆超限时**替换 `last_seen` 最旧条目**，不拒绝新配对 | 拒绝配对会让免费版显得残废（P4） |
| D6 | 会话内 `start_with_reconnect` 容错**保持免费**，Pro 只卖「跨启动的自动重连上次设备」 | 断线容错属基本可靠性，收费不合理 |
| D7 | 校验失败**降级为免费版，绝不阻止启动或中断音频**（E1） | 付费用户被自己的软件锁死是最严重信任事故 |
| D8 | `Ctrl+Shift+S`（显示主窗口）保持免费，`Ctrl+Shift+P` 与新增收发直控快捷键归 Pro | 「找回被最小化的窗口」是基本可用性 |

### license 规格要点

- 格式：`SLPRO-<base32(payload_json)>-<base32(ed25519_sig)>`
- payload 字段：`v` / `sku` / `iat` / `exp` / `sub` / `bind` / `seats` / `nonce`
- 设备指纹：`base32(sha256("soundlink-fp-v1" || machine_id || device_id))[..10]`
- 验签所需依赖（`ed25519-dalek` / `base64` / `sha2` / `serde_json` / `keyring`）**全部已在**，无需新增第三方依赖

---

## 5. 现状基线（已读代码，供后续实装参考）

| 关注点 | 现状 | 位置 |
|---|---|---|
| 三项自动化开关 | 已实装（配置 + `get/set_app_settings` + autostart 注册项同步） | [`commands/mod.rs`](../../../desktop/src-tauri/src/commands/mod.rs#L930-L1037) |
| 自动收发触发 | 前端驱动，mount 时读设置后调 `start_receiver` / `connect_trusted_receiver` | [`App.tsx`](../../../desktop/ui/src/App.tsx#L316-L360) |
| 自动发送目标 | 取 `list_trusted_receivers` **第一个**有 host/port 的条目，无「上次设备」概念 | [`App.tsx`](../../../desktop/ui/src/App.tsx#L340-L351) |
| 信任存储 | `TrustStore` JSON，**无数量上限** | [`trust_store.rs`](../../../desktop/src-tauri/src/pairing/trust_store.rs#L36-L100) |
| 全局快捷键 | `main.rs` 内无条件注册 | [`main.rs`](../../../desktop/src-tauri/src/main.rs#L50-L77) |
| 断线重连 | 发送端有 `start_with_reconnect`，无「启动时重连上次设备」 | [`sender.rs`](../../../desktop/src-tauri/src/sender.rs#L229) |
| feature 体系 | `default = []` / `opus` / `wasapi` / `tauri_app`；**不新增 `pro` feature**（改目录替换） | [`Cargo.toml`](../../../desktop/src-tauri/Cargo.toml) |
| Cargo 结构 | **不是 workspace**，`desktop/src-tauri` 为独立 crate（`soundlink` / lib `soundlink_lib`）；`shared/` 下无任何 Cargo.toml | — |
| 配置目录 | `dirs::config_dir()/soundlink`（Windows = `%APPDATA%\soundlink`），与安装目录分离 | [`main.rs`](../../../desktop/src-tauri/src/main.rs#L165-L171) |
| keyring | service `"soundlink"`，identity account `"device_identity_ed25519"`；失败回退文件 | [`device_identity.rs`](../../../desktop/src-tauri/src/device/device_identity.rs) |
| 自动更新 | **未接入 `tauri-plugin-updater`**，`tauri.conf.json` 无 updater 段 | — |

---

## 6. 回填与版本

- [`docs/First/12-plan.md`](../../First/12-plan.md) 发布侧进度已加商业化指针：方案定稿 ✅ 2026-08-06；构建方案 V-1~V-5 实测通过 ✅ 2026-08-06；Q/R/S/T/U 全部 ⬜。
- `CHANGELOG.md [未发布]` 已记录规划文档新增，并 ⚠ 标注两项归属调整（三项自动化开关移入 Pro、免费版设备记忆上限 1 台）；第二轮追加记录跨版本兼容约束、多仓库指南与 ⚠ 构建方式定稿调整（不用 `--features pro`）；第三轮追加实测确认与三条构建硬约束（版本号一致 / `cargo clean -p` / 相对路径）。
- **建议版本级别：不升版本**（本次仅新增与修订规划文档，无用户可感知的功能变更；实测在临时目录进行，未改动仓库代码）。后续里程碑对应：M-B/M-C → `v0.2.0`；M-D/M-E → `v0.3.0`（首个含 Pro 版本，建议 **MINOR**）。
- 未修改 `VERSION`，未执行 `git commit`。

---

## 7. 用户需自行完成的部分

1. 确认 `00-monetization-overview.md` §2–§7 决策后方可进入阶段 Q（M-A 里程碑）。
2. 创建私有仓库 `soundlink-pro` 与 CI deploy key。
3. 生成并离线保管 license 签发私钥（泄露即全盘失效）。
4. 爱发电档位与淘宝小店上架、README/官网加 open-core 说明。

---

## 8. 已知边界

- 移动端不涉及本次收费设计。
- macOS 采集未实装，Pro 的自启/托盘在 macOS 上的行为待该平台就绪后再评估。
- 方案未包含任何反破解强化（刻意不做，与零遥测承诺一致）；key 泄露只走本地吊销名单。
- `02` 文档的构建方案（目录替换）依赖「Cargo 会解析可选依赖」这一判断，**尚未实测**；V-1 结论若相反可回退 feature 方案，但届时须同步改 `00`/`01`/`02` 三份文档。
- 移动端不参与 Pro，因此多仓库切分只影响桌面端；`pubspec.yaml` 与 `sync_version.py` 现有 4 个 TARGETS 均不需改动。

---

## 9. 关联文档

- 收费框架：[`00-monetization-overview.md`](../../NewFunctions/monetization/00-monetization-overview.md)
- 工程方案：[`01-engineering-plan.md`](../../NewFunctions/monetization/01-engineering-plan.md)
- 多仓库构建与使用：[`02-multi-repo-guide.md`](../../NewFunctions/monetization/02-multi-repo-guide.md)
- 竞品与定价参照：[`../../NewFunctions/opensource-launch/01-market-research.md`](../../NewFunctions/opensource-launch/01-market-research.md)
- 版本递增判定：[`../../NewFunctions/version-management/01-versioning-policy.md`](../../NewFunctions/version-management/01-versioning-policy.md)
- 前序会话：[FT-0015](./0015-2026-08-03-opensource-launch-audit.md)（开源发布审计）、[FT-0019](./0019-2026-08-05-ui-simplify-settings-migration.md)（设置页迁移，Pro 开关的 UI 落点）
