<!-- VER-00 -->
# 版本管理架构与实现计划

> 建档：2026-08-04 · 对象：整个仓库的**版本号定义、同步、校验、发布与变更日志**
> 触发背景：提交 `96e3685`（`mds: fix to release`）补齐了发布前文档与 Release 工作流，但审计发现仓库仍**只有散落的版本号字段，没有版本管理机制，也没有任何自动化**。首个 tag `v0.1.0-beta`（OSL-K3）尚未打出，是引入规范的最佳时机。

---

## 1. 与其他规划目录的分工

| 目录 | 回答的问题 |
|---|---|
| `release-readiness/` | 产品能不能发（功能、安全、跨平台、测试） |
| `opensource-launch/` | 怎么发、发给谁（仓库配套、Release 流水线、市场） |
| `version-management/`（本目录） | **版本号从哪来、怎么保持一致、发版时哪些步骤必须自动化** |

本目录是 OSL 阶段 K（Release 流水线）的**前置依赖**：K3 打 tag 之前应先落地 V1–V4，否则首个 Release 就会带着「tag 是 `v0.1.0-beta`、安装包内嵌 `0.1.0`、Android 显示 `1.0.0`」的三方不一致。

---

## 2. 现状审计（2026-08-04）

### 2.1 版本号字段分布

| 位置 | 当前值 | 用途 | 维护方式 |
|---|---|---|---|
| [`desktop/src-tauri/Cargo.toml`](../../../desktop/src-tauri/Cargo.toml) `version` | `0.1.0` | `CARGO_PKG_VERSION` → 关于页 | 手工 |
| [`desktop/src-tauri/tauri.conf.json`](../../../desktop/src-tauri/tauri.conf.json) `version` | `0.1.0` | NSIS 安装包版本、exe 文件属性 | 手工 |
| [`desktop/ui/package.json`](../../../desktop/ui/package.json) `version` | `0.1.0` | 仅 npm 包元数据，未对外展示 | 手工 |
| [`website/package.json`](../../../website/package.json) `version` | `0.1.0` | 官网站点包元数据，与产品无关 | 手工 |
| [`mobile/flutter_app/pubspec.yaml`](../../../mobile/flutter_app/pubspec.yaml) `version` | `1.0.0+1` | → Android `versionName` / `versionCode`、iOS `CFBundleShortVersionString` | 手工 |
| `desktop/src-tauri/src/constants.rs` `PROTOCOL_VERSION` | `1` (u8) | 音频包头校验 | 手工 |
| `mobile/flutter_app/lib/src/constants.dart` 协议版本 | `1` | 同上（双端各自硬编码） | 手工 |

### 2.2 已有的自动化（仅一处）

- [`desktop/src-tauri/build.rs`](../../../desktop/src-tauri/build.rs)：注入 `BUILD_DATE`，关于页显示构建日期。
- [`commands/mod.rs::get_app_version`](../../../desktop/src-tauri/src/commands/mod.rs)：返回 `CARGO_PKG_VERSION` / `BUILD_DATE` / 许可证 / 仓库链接。

### 2.3 问题清单

| 编号 | 问题 | 影响 |
|---|---|---|
| P-1 | **移动端与桌面端版本号差一个大版本**（`1.0.0+1` vs `0.1.0`），`1.0.0` 是 Flutter 模板默认值，从未被修正 | 用户看到 Android 显示「1.0.0」会误认为已是正式版，与 README「Early Access / Pre-release」定位直接冲突 |
| P-2 | **仓库无任何 git tag**（`git tag --list` 为空） | 无版本锚点，`git describe` 不可用，无法生成区间变更日志 |
| P-3 | **Release 工作流不校验 tag 与内嵌版本一致性**：[`release.yml`](../../../.github/workflows/release.yml) 仅用 `GITHUB_REF_NAME` 给产物**改名**，安装包内部仍是 `tauri.conf.json` 里的旧版本；NSIS 产物因此只能靠 `ls *.exe \| head -n 1` 通配匹配 | 产物文件名说 `v0.2.0`、控制面板里显示 `0.1.0`；通配匹配在出现多个 exe 时会静默拿错文件 |
| P-4 | **无 bump / 同步脚本**（仓库无 `scripts/` 目录） | 每次发版需人工改 5 个文件，漏改必然发生（P-1 即为例证） |
| P-5 | **CHANGELOG 全手工**：`[未发布]` → `[x.y.z] - 日期` 的改写、以及新建空 `[未发布]` 均为人工步骤 | 易漏；且发版时 Release Notes 与 CHANGELOG 内容需二次手抄（`release.yml` 的 Notes 目前是**硬编码文本**，不从 CHANGELOG 提取） |
| P-6 | **Android `versionCode` 无递增策略**：来自 pubspec 的 `+1`，从未变更 | 一旦有第二个 APK，`versionCode` 相同将无法覆盖安装；未来上架商店会被拒 |
| P-7 | **协议版本与产品版本无关联管理**，双端各自硬编码常量，改协议时靠人记得改两处 | 违反 `shared/` 单源原则；跨版本兼容矩阵无处可查 |
| P-8 | **移动端不展示 App 版本**：`settings_page.dart` 只显示「协议版本 v1」 | 用户报 Bug 时无法说明所用版本，Issue 模板要求的「版本」字段填不出来 |
| P-9 | **无应用内更新提示** | 用户装了 beta 后不知道有新版；LAN 工具尤其需要双端版本匹配提示 |
| P-10 | **无版本兼容性协商**：`PROTOCOL_VERSION` 不匹配只报 `versionMismatch(1003)`，不提示「请升级哪一端」 | 用户面对错误码无法自助解决 |

**结论：仓库有「版本号」但没有「版本管理」，自动版本管理为零。**

---

## 3. 目标架构

### 3.1 三个独立的版本域

必须区分，不可混用：

| 版本域 | 名称 | 语义 | 变更节奏 | 单一来源 |
|---|---|---|---|---|
| 产品版本 | `PRODUCT_VERSION` | 用户可见的发布版本，SemVer | 每次 Release | 根 `VERSION` 文件 |
| 协议版本 | `PROTOCOL_VERSION` | 线上格式兼容性，单调整数 | 仅当报文格式不兼容变更 | `shared/constants` |
| 构建号 | `BUILD_NUMBER` | 移动端商店要求的单调递增整数 | 每次 CI 构建 | CI 计算，不入库 |

关键约束：**产品版本升级不得自动升协议版本**；协议版本升级**必须**同步 [`docs/First/04-protocol.md`](../../First/04-protocol.md) 与 `11-implementation-spec.md`（项目规则硬红线）。

### 3.2 单一版本源（SSOT）

在仓库根新增 `VERSION` 文件，内容为一行纯 SemVer：

```
0.1.0-beta.1
```

选择纯文本而非 `version.json` 的理由：任何语言/脚本/CI 都能一行读取，无解析依赖，diff 干净。

**同步目标（由脚本写入，禁止手改）：**

| 目标文件 | 写入字段 | 转换规则 |
|---|---|---|
| `desktop/src-tauri/Cargo.toml` | `[package] version` | 原样（Cargo 支持 SemVer 预发布） |
| `desktop/src-tauri/tauri.conf.json` | `version` | 原样 |
| `desktop/ui/package.json` | `version` | 原样 |
| `mobile/flutter_app/pubspec.yaml` | `version` | `<核心三段>+<BUILD_NUMBER>`，预发布后缀**丢弃**（见 §3.3） |

**明确排除：** `website/package.json` 不参与同步——官网是独立可部署站点，与客户端版本无耦合，固定 `0.0.0` 或保留现值即可，避免误导。

### 3.3 移动端版本号转换规则

Android `versionName` / iOS `CFBundleShortVersionString` 不接受 `-beta.1` 这类后缀（Android 可显示但商店排序混乱，iOS 直接拒绝非纯数字点分）。规则：

- `versionName` = SemVer 的 `major.minor.patch`，**去掉预发布后缀**。
- 预发布信息通过 `BUILD_NUMBER` 与 Release 页说明体现，不进 `versionName`。
- `BUILD_NUMBER` 计算：`major*10000 + minor*100 + patch` 为基数，加 CI 递增量；首发可直接用 CI `github.run_number`。要求只有一条：**单调递增**。
- 落地第一步即修正 P-1：`1.0.0+1` → `0.1.0+<N>`。

### 3.4 协议版本与产品版本的兼容矩阵

在本目录维护 `01-compatibility-matrix.md`（V7 交付），记录每个产品版本对应的 `PROTOCOL_VERSION`，以及「哪些版本组合可互通」。这是 LAN 双端软件的必要文档：用户必然出现「手机装了新版、电脑还是旧版」。

---

## 4. 自动化方案选型

| 方案 | 适配度 | 结论 |
|---|---|---|
| 现状（纯手工） | — | 已产生 P-1，不可接受 |
| `cargo-release` / `release-plz` | 仅覆盖 Rust crate | ❌ 管不到 pubspec / tauri.conf.json，异构仓库主体在 Rust 之外 |
| `changesets` | 仅覆盖 npm 工作区 | ❌ 同上，且本仓库不是 monorepo npm 结构 |
| `semantic-release` + 插件 | 能力足够 | ❌ 需引入 Node 全局依赖链与 commit message 强约束（当前 [`CONTRIBUTING.md`](../../../CONTRIBUTING.md) 只要求「祈使句说明为什么」，非 Conventional Commits），改造成本与收益不匹配 |
| **自研 Python 脚本 + CI 校验门** | 高 | ✅ **采用** |

**采用理由：** 仓库跨 Rust / npm / Dart / Gradle 四种生态，任何单生态工具都覆盖不全；项目已约定 Python 使用根目录 `.venv`（项目规则），脚本在 Windows 本地与 Linux CI 上行为一致；逻辑总量小（读一行、改四处、校验一次），无需引入重依赖。

**实现约束：**
- 脚本置于 `scripts/`，用 `.venv` 的 Python 执行，**禁止**系统 Python 装包（项目规则）。
- 只用标准库：`tomllib`（读）+ 正则/行级替换（写）。**不引入 `toml`/`ruamel.yaml` 等写库**——目标字段都是单行，行级替换足够，且能保留原文件注释与格式。
- 脚本必须支持 `--check`（只校验、不写、不一致时非零退出），供 CI 使用。

---

## 5. 实现计划（阶段 V）

| 任务 | 说明 | 优先级 | 状态 |
|---|---|---|---|
| V1 · 建立 SSOT | 根 `VERSION` 文件，初值 `0.1.0-beta.1`；在 `AGENTS.md` 常见任务表加「改版本号 → `VERSION` + `scripts/sync_version.py`」指针 | 必做 | [x] — 2026-08-04 `VERSION` 已建，AGENTS.md 表已加指针 |
| V2 · 同步脚本 | `scripts/sync_version.py`：读 `VERSION` → 写 Cargo.toml / tauri.conf.json / desktop ui package.json / pubspec.yaml；支持 `--check` 与 `--build-number N` | 必做 | [x] — 2026-08-04 标准库实现（tomllib 读 + 行级替换写），保留原文件格式 |
| V3 · 修正现存不一致 | 执行 V2 脚本，把 pubspec 从 `1.0.0+1` 拉回与桌面端一致（修 P-1；P-6 仅备好 `--build-number` 能力，接线见 V15） | 必做 | [x] — 2026-08-04 4 个目标已同步到 `0.1.0-beta.1`，pubspec 改为 `0.1.0+1`；Cargo.lock 中 soundlink 条目同步。**P-6 未闭环**：`release.yml` 未调用 `--build-number`，`versionCode` 仍固定，转 V15 |
| V4 · CI 一致性门 | `ci.yml` 增加 `version-check` job：跑 `sync_version.py --check`，不一致直接失败 | 必做 | [x] — 2026-08-04 `version-check` job 已加，setup-python 3.12 |
| V5 · Release 工作流对齐 | `release.yml` 增加步骤：校验 `VERSION` 与 tag（`v` + `VERSION` 必须相等）→ 不等则 fail；产物收集改用确定文件名而非 `ls \| head`（修 P-3） | 必做 | [x] — 2026-08-04 抽出 `version-gate` job（两个 build job `needs` 它），NSIS 收集改用「找到且只有一个 setup.exe」断言 |
| V6 · Release Notes 自动提取 | 从 `CHANGELOG.md` 抽取当前版本小节作为 Release body，保留三条免责声明为固定前言（修 P-5 的手抄环节） | 建议 | [ ] |
| V7 · 兼容矩阵文档 | 本目录 `02-compatibility-matrix.md`：版本 × `PROTOCOL_VERSION` × 可互通组合（修 P-7 的可查性） | 建议 | [ ] |
| V8 · 协议版本单源化 | `PROTOCOL_VERSION` 由 `shared/constants` 生成或以测试断言双端一致，避免单端漏改（修 P-7） | 建议 | [ ] |
| V9 · 移动端展示 App 版本 | `settings_page.dart` 关于区增加「应用版本」（与「协议版本」并列），版本值取构建期注入（修 P-8） | 建议 | [ ] |
| V10 · 版本不匹配的用户可读提示 | 协议版本不匹配时，除错误码 1003 外提示「请升级发送端/接收端」，并在 UI 显示对端版本（修 P-10） | 建议 | [ ] |
| V11 · 应用内更新检查 | 桌面端查询 GitHub Releases latest，有新版则在设置页提示（**仅提示不自动下载**，避免签名缺失下的自更新风险）；需明确隐私影响并写入 `docs/privacy.md`（修 P-9） | 后续 | [ ] |

> **V12–V15 见 [`01-versioning-policy.md`](./01-versioning-policy.md) §6**：版本语义与递增判定规则、AI 工作流版本维护义务、规则文件挂钩、以及 V15（P-6 接线）。本文档负责「怎么改版本号」，`01` 负责「何时改、改哪一位、谁来改」。

### 5.1 依赖顺序

```
V1 → V2 → V3 → V4 → V5 → V6
              ↘ V7 → V8
                     V9 → V10 → V11
```

V1–V5 是 **OSL-K3（首个 tag）的前置**，必须先完成。V6–V10 可在 `v0.1.0-beta` 之后补。V11 排到 `v0.2.0` 及以后，且必须在代码签名之前保持「只提示」。

---

## 6. 发布流程（V1–V6 落地后的目标形态）

```powershell
# 1. 确定新版本，写入 SSOT
# （手工编辑 VERSION，例如 0.1.0-beta.1 → 0.1.0）

# 2. 同步到各生态清单
.venv\Scripts\python.exe scripts\sync_version.py

# 3. 定稿 CHANGELOG：[未发布] → [0.1.0] - YYYY-MM-DD，并新建空 [未发布]

# 4. 本地验证门（与 CI 一致）
.venv\Scripts\python.exe scripts\sync_version.py --check

# 5. 提交 + 打 tag（由用户本人执行，项目规则禁止代理提交/推送）
#    tag 名必须是 v + VERSION 内容，否则 release.yml 会 fail

# 6. push tag → CI 构建 → 核对 Draft Release → 手动 Publish
```

对比现状省掉的人工环节：改 4 个文件、抄 Release Notes、以及「记得 tag 和内嵌版本要一致」这个纯靠人记的约束。

---

## 7. 风险与边界

| 风险 | 缓解 |
|---|---|
| 行级正则替换误伤同名字段（如 `Cargo.toml` 中依赖项的 `version = `） | 脚本必须限定作用域：`[package]` 段内首个 `version`；`pubspec.yaml` 顶层 `version:`；`package.json` 顶层 `"version"`。写完后用 `--check` 自校验，并纳入 V4 的 CI 门 |
| `--check` 门收紧后，历史提交在 CI 上批量变红 | V3 先一次性修正到一致，再开 V4 |
| iOS 版本号可能另有硬编码 | 已核实：`ios/Runner/Info.plist` 用 `$(FLUTTER_BUILD_NAME)` / `$(FLUTTER_BUILD_NUMBER)`，随 pubspec 自动注入，**无需额外同步目标** |
| 协议版本被误当产品版本升级 | §3.1 明确隔离；V8 加双端一致性断言；改协议须同步 04 与 11（项目规则硬红线） |
| 自更新引入供应链风险（安装包尚未签名） | V11 仅做「提示 + 跳转 Releases 页」，不实现静默下载安装；签名完成前不放开 |
| Android 首个正式签名版与 debug 签名 beta 不可覆盖升级 | 已是既有已知限制（见 `release.yml` 注释与 OSL 风险表），版本策略层面额外说明：正式签名切换时 minor 至少 +1 并在 CHANGELOG 标「需卸载重装」 |

---

## 8. 回填规则（强约束）

1. 完成任一任务后立即把 `[ ]` 改为 `[x]`，行末补 `— YYYY-MM-DD 备注`。
2. 阶段全部完成后，同步 [`../opensource-launch/00-launch-overview.md`](../opensource-launch/00-launch-overview.md) §2 与 [`../release-readiness/00-release-overview.md`](../release-readiness/00-release-overview.md) §2/§4。
3. 涉及项目阶段推进时，回填 [`../../First/12-plan.md`](../../First/12-plan.md)。
4. 验收未过不得标完成。
5. 状态取值：`⬜ 未开始` / `🟡 进行中` / `✅ 完成` / `⏸ 暂停`。

---

## 9. 关联文档

- **版本语义与递增判定 + AI 工作流义务：[`01-versioning-policy.md`](./01-versioning-policy.md)**
- 开源发布与 Release 流水线：[`../opensource-launch/00-launch-overview.md`](../opensource-launch/00-launch-overview.md)（阶段 K）
- 产品发布就绪度与版本里程碑：[`../release-readiness/00-release-overview.md`](../release-readiness/00-release-overview.md) §4
- 协议定义：[`../../First/04-protocol.md`](../../First/04-protocol.md)
- 编码规格：[`../../First/11-implementation-spec.md`](../../First/11-implementation-spec.md)
- 变更日志与回填规则：[`../../../CHANGELOG.md`](../../../CHANGELOG.md)
- CI / Release 工作流：`.github/workflows/ci.yml`、`.github/workflows/release.yml`
