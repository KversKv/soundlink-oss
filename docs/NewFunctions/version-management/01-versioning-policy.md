<!-- VER-01 -->
# 版本号语义与递增决策规则（含 AI 工作流版本意识）

> 建档：2026-08-04 · 对象：**判定「该不该升版本、升哪一位」**，以及代理在日常任务中必须承担的版本维护义务
> 前置：[`00-version-management-plan.md`](./00-version-management-plan.md) 已落地 V1–V5（SSOT + 同步脚本 + CI 门），解决了「怎么改」；本文解决「**什么时候改、改哪一位、谁来改**」。

---

## 1. 为什么需要本文

V1–V5 交付后审计发现：代理完整实现了版本管理系统，却**没有回填 `CHANGELOG.md`**，也没有同步 OSL 总览。原因是规则缺位——`AGENTS.md` 与 `.trae/rules/project-rules.md` 中：

- 版本相关约束**仅一行**，且只说「用 `scripts/sync_version.py`」（怎么改），未说何时改、改哪一位。
- `CHANGELOG` 在两份规则文件中**出现 0 次**；强制回填约束只覆盖 `docs/First/12-plan.md` 与 FunctionTasks 归档。

即：**工具有了，意识没有**。本文补齐语义规则（§2–§4）与代理义务（§5）。

---

## 2. 版本号构成

```
MAJOR . MINOR . PATCH  [ -PRERELEASE ]
  │       │       │         │
  │       │       │         └─ 预发布：beta.N / rc.N（仅产品版本域）
  │       │       └─ 修订号（小版本中最小粒度）
  │       └─ 次版本号（口语「小版本」）
  └─ 主版本号（口语「大版本」）
```

术语对照（避免沟通歧义，本项目内统一按下表理解）：

| 口语说法 | 对应位 | 例 |
|---|---|---|
| **大版本** | MAJOR | `0.x` → `1.0.0` |
| **小版本** | MINOR | `0.1.0` → `0.2.0` |
| **修订 / 补丁** | PATCH | `0.1.0` → `0.1.1` |
| 预发布迭代 | PRERELEASE | `0.1.0-beta.1` → `0.1.0-beta.2` |

> 注意：本文所有规则针对**产品版本域**。协议版本（`PROTOCOL_VERSION`，单调整数）与构建号（`BUILD_NUMBER`）是独立版本域，规则见 [`00-version-management-plan.md`](./00-version-management-plan.md) §3.1，**不得联动升级**。

---

## 3. 递增判定规则

### 3.1 判定优先级（自上而下，命中即停）

按顺序问以下问题，命中第一个即确定级别：

| 序 | 判定问题 | 命中则 |
|---|---|---|
| 1 | 用户升级后**必须重新配对、卸载重装、或旧端无法再连**？ | **MAJOR** |
| 2 | `PROTOCOL_VERSION` 变更且**不向后兼容**？ | **MAJOR** |
| 3 | 新增用户可感知的功能、平台、设置项，或协议**向后兼容**扩展？ | **MINOR** |
| 4 | 仅修 Bug、性能、文案、依赖升级、内部重构？ | **PATCH** |
| 5 | 仅改文档 / CI / 注释，无产物行为变化？ | **不升版本** |

### 3.2 各级别的具体触发项（SoundLink 场景）

**MAJOR（大版本）— 破坏性，用户必须做动作**

- 音频包头 / 控制消息格式不兼容变更（旧发送端连不上新接收端）
- 配对密钥格式、keyring 存储 schema 不兼容变更（用户需重新配对）
- Android 签名从 debug 切换为正式签名（**无法覆盖安装，必须卸载重装**）
- 配置文件格式不兼容且无迁移逻辑
- 移除已发布的功能或平台支持
- `0.x` → `1.0.0`：承诺 API/协议稳定、跨平台补全、代码签名到位（见 [`../release-readiness/00-release-overview.md`](../release-readiness/00-release-overview.md) §4）

**MINOR（小版本）— 加东西，向后兼容**

- 新增平台支持（如 macOS 采集、Linux 输出）
- 新增用户可见功能（i18n、应用内更新提示、快捷键新增）
- 新增设置项 / 新增可选协议字段（旧端忽略即可正常工作）
- 协议版本 +1 但**保留旧版本兼容处理**

**PATCH（修订）**

- Bug 修复（音频噪声、断线重连失败、UI 错位）
- 性能优化、延迟调优（不改默认基线值）
- 文案修正、错误提示改善
- 依赖版本升级（无行为变化）

**不升版本**

- 纯文档、注释、`docs/` 变更
- CI 工作流调整（不影响产物）
- 测试补充

### 3.3 `0.x` 阶段特殊规则（当前适用）

当前 `VERSION = 0.1.0-beta.1`，处于 `0.x` 阶段。按 SemVer 第 4 条，`0.x` 阶段允许破坏性变更走 MINOR：

> **在 `1.0.0` 发布前：本应触发 MAJOR 的破坏性变更，降级为 MINOR（`0.1.0` → `0.2.0`）。**

但**约束不变**：破坏性变更**必须**在 `CHANGELOG.md` 中以醒目方式标注（如「⚠ 需重新配对」「⚠ 需卸载重装」），不因降级为 MINOR 而降低告知力度。这是 `0.x` 阶段唯一的妥协点——**版本号可以宽松，告知不可以**。

### 3.4 预发布号递增

| 场景 | 变化 |
|---|---|
| beta 期间修 Bug / 补功能 | `0.1.0-beta.1` → `0.1.0-beta.2` |
| beta 转候选发布 | `0.1.0-beta.N` → `0.1.0-rc.1` |
| 转正式 | `0.1.0-rc.N` → `0.1.0` |
| beta 期间发生**破坏性**变更 | 先升 MINOR 再重开 beta：`0.1.0-beta.2` → `0.2.0-beta.1` |

移动端 `versionName` 丢弃预发布后缀（iOS 拒绝非纯数字点分），预发布信息通过 `BUILD_NUMBER` 与 Release 页体现，详见 plan §3.3。

---

## 4. 判定示例（对照表）

| 变更 | 级别 | 理由 |
|---|---|---|
| 修复 Opus 解码噪声 | PATCH | 纯 Bug 修复 |
| 新增 macOS 系统音频采集 | MINOR | 新增平台支持 |
| 音频包头增加可选字段，旧端忽略仍可用 | MINOR | 协议向后兼容扩展 |
| 音频包头字段重排，旧端解析失败 | MAJOR（`0.x` 期降 MINOR + ⚠ 标注） | 协议不兼容 |
| Android 切正式签名 | MAJOR（`0.x` 期降 MINOR + ⚠ 需卸载重装） | 用户必须做动作 |
| 默认 Jitter 从 80ms 调为 60ms | PATCH | 调优，无兼容性影响（但属音频基线，须先按项目规则确认） |
| UI 增加英文 i18n | MINOR | 新增用户可感知功能 |
| 补充 `docs/` 文档 | 不升 | 无产物行为变化 |
| 本次 V1–V5 版本管理系统落地 | 不升版本，但**必须写 CHANGELOG** | 改变构建/发布方式，无产物行为变化 |

最后一行是本次审计的实际教训：**「不升版本」≠「不用记录」**，二者是独立判断。

---

## 5. AI 工作流的版本维护义务（强约束）

代理在每次任务收尾时，**必须**依次完成以下检查。这是与「进度回填约束」同级的硬约束。

### 5.1 义务清单

| 序 | 义务 | 判定 | 违反后果 |
|---|---|---|---|
| A | **CHANGELOG 回填**：任何用户可感知变更、或影响构建/使用方式的变更，写入 `CHANGELOG.md` 的 `[未发布]` 对应小节（新增/变更/修复/移除/安全） | 见 §5.2 决策树 | 发版时遗漏条目，Release Notes 不完整 |
| B | **bump 级别建议**：在 FunctionTasks 归档中显式写出「本次变更建议的版本级别（MAJOR/MINOR/PATCH/不升）+ 理由」 | 每次归档必写 | 人类无从判断累积变更该发什么版本 |
| C | **禁止自行改 `VERSION`** | 无例外 | 见 §5.3 |
| D | **禁止手改 4 个清单的 version 字段**：必须走 `scripts/sync_version.py` | 无例外 | 绕过 SSOT，CI 门会拦下 |
| E | **协议变更连带检查**：改动 `PROTOCOL_VERSION` 或报文格式时，必须评估是否触发 §3.2 MAJOR，并同步 `docs/First/04-protocol.md` 与 `11-implementation-spec.md` | 涉协议必查 | 违反项目规则硬红线（单源原则） |
| F | **破坏性变更醒目标注**：`0.x` 阶段降级为 MINOR 时，CHANGELOG 条目必须带 ⚠ 与用户需执行的动作 | 涉破坏性必查 | 用户升级后功能静默失效 |

### 5.2 CHANGELOG 回填决策树

```
本次改动是否产生用户可感知的差异？
├─ 是 → 写 CHANGELOG [未发布]
└─ 否 → 是否改变了构建方式 / 发布方式 / 使用方式？
        ├─ 是 → 写 CHANGELOG [未发布]（例：本次 V1–V5）
        └─ 否 → 是否仅文档 / 注释 / 测试 / 纯内部重构？
                ├─ 是 → 不写
                └─ 否 → 写（存疑时一律写；漏记的代价大于多记）
```

### 5.3 为什么禁止代理自行 bump `VERSION`

`VERSION` 的修改等价于**宣布发版意图**，其下游连锁反应包括：tag 名称约定、CI `version-gate` 校验、Release 产物命名、用户可见版本号、以及移动端 `versionCode` 的不可回退性（商店版本号只能升不能降）。

代理无法判断「本轮改动是否已构成一次发布」——这是产品决策，非工程决策。因此：

- **代理做**：维护 `CHANGELOG.md [未发布]`（持续累积）+ 在归档中给出 bump 级别建议。
- **人类做**：决定何时发版、编辑 `VERSION`、执行 `sync_version.py`、打 tag、push。

这与项目规则「禁 `git commit`（除非明确要求）」同源——**发布动作归人类**。

若用户**明确要求**代理修改 `VERSION`，则代理执行时必须：读 `VERSION` → 按 §3 判定级别并说明理由 → 改 `VERSION` → 跑 `sync_version.py` → 跑 `--check` 自验 → 提示用户「tag 名必须为 `v` + VERSION 内容」。

### 5.4 收尾自检（每次任务结束前逐项确认）

- [ ] 是否需要写 `CHANGELOG.md [未发布]`？（走 §5.2 决策树，勿跳过）
- [ ] FunctionTasks 归档中是否写了 bump 级别建议 + 理由？
- [ ] 是否手改了 4 个清单的 version 字段？（若改过，改回并走脚本）
- [ ] 若动了 `VERSION`：是否跑过 `sync_version.py --check`？
- [ ] 若动了协议：是否同步了 `04-protocol.md` 与 `11-implementation-spec.md`？是否评估了 MAJOR？
- [ ] 若含破坏性变更：CHANGELOG 是否带 ⚠ 与用户需执行的动作？
- [ ] 是否回填了 `docs/First/12-plan.md` 与本目录 / OSL / release-readiness 的相关总表？

---

## 6. 落地任务

| 任务 | 说明 | 优先级 | 状态 |
|---|---|---|---|
| V12 · 本文档 | 版本语义 + 递增判定 + AI 工作流义务 | 必做 | [x] — 2026-08-04 |
| V13 · 规则文件挂钩 | `AGENTS.md` 的「进度回填约束」升级为「进度与版本回填约束」，写入 §5.1 义务 A–F 摘要与本文指针；`project-rules.md` 硬红线加一条（受 <1000 字符限制，仅留指针） | 必做 | [x] — 2026-08-04 AGENTS.md 新增「版本维护义务」表（A–F）；project-rules.md 硬红线加「禁自行改 VERSION」+ 流程加 CHANGELOG 回填，全文 979 字符符合元规则 |
| V14 · 补回本次遗漏 | 把 V1–V5 写入 `CHANGELOG.md [未发布]`；同步 OSL 总览 §2 | 必做 | [x] — 2026-08-04 CHANGELOG 新增/变更小节各补 2 条与 4 条；OSL §2 K 行、K3 行、K3 实操命令、发布前清单同步（tag 名修正为 `v0.1.0-beta.1`） |
| V15 · P-6 接线 | `release.yml` Android 构建改用 `--build-number ${{ github.run_number }}`，使 `versionCode` 真正单调递增（V3 标完成但未接线） | 必做 | [x] — 2026-08-04 `mobile-android` job 增加 setup-python + Inject monotonic build number 步骤，置于 `flutter pub get` 之前 |

---

## 7. 关联文档

- 版本管理架构与实现计划：[`00-version-management-plan.md`](./00-version-management-plan.md)
- 兼容矩阵（V7 待交付）：`02-compatibility-matrix.md`
- 变更日志与其自身回填规则：[`../../../CHANGELOG.md`](../../../CHANGELOG.md)
- 代理工作说明：[`../../../AGENTS.md`](../../../AGENTS.md)
- 协议定义：[`../../First/04-protocol.md`](../../First/04-protocol.md)
- 编码规格：[`../../First/11-implementation-spec.md`](../../First/11-implementation-spec.md)
- 版本里程碑：[`../release-readiness/00-release-overview.md`](../release-readiness/00-release-overview.md) §4
