<!-- FT-0016 -->
# 版本管理系统落地实录：V1–V5 + V12–V15（2026-08-04）

> 场景：仓库已有散落的版本号字段但无版本管理机制，移动端 `1.0.0+1` 与桌面端 `0.1.0` 差一个大版本（P-1），首个 tag `v0.1.0-beta` 尚未打出。按 [`docs/NewFunctions/version-management/00-version-management-plan.md`](../../../docs/NewFunctions/version-management/00-version-management-plan.md) 落地 V1–V5（OSL-K3 前置）。

## 背景

审计发现仓库存在 10 项版本管理缺陷（P-1 ~ P-10），核心症状：

- 移动端 `pubspec.yaml` 是 Flutter 模板默认值 `1.0.0+1`，从未修正，与桌面端 `0.1.0` 差一个大版本。
- 无 `git tag`，`git describe` 不可用。
- `release.yml` 用 `ls *.exe | head -n 1` 通配匹配 NSIS 产物，多个 exe 时会静默拿错。
- 改版本号需手改 5 个文件，漏改必然发生。

## 方案分析

参见 plan §4 选型：跨 Rust / npm / Dart / Gradle 四生态，任何单生态工具（cargo-release / changesets / release-plz）都覆盖不全；semantic-release 需引入 Node 全局依赖链且强约束 Conventional Commits（仓库当前只要求祈使句说明）。最终采用**自研 Python 脚本 + CI 校验门**：标准库实现，无重依赖，跨平台行为一致。

三个独立版本域（plan §3.1）：产品版本（SemVer）/ 协议版本（单调整数）/ 构建号（移动端单调递增）。本次仅落地产品版本域的 SSOT 与同步链路，协议版本单源化（V8）排到后续。

## 实现清单

| 任务 | 交付物 | 关键文件 |
|---|---|---|
| V1 SSOT | 仓库根 `VERSION` 文件，初值 `0.1.0-beta.1` | [VERSION](../../../VERSION) |
| V1 指针 | AGENTS.md 常见任务表加「改版本号」行 | [AGENTS.md](../../../AGENTS.md) |
| V2 同步脚本 | 标准库实现，支持 `--check` / `--build-number N` | [scripts/sync_version.py](../../../scripts/sync_version.py) |
| V3 修正不一致 | 4 个清单同步到 `0.1.0-beta.1`；pubspec `1.0.0+1` → `0.1.0+1`；Cargo.lock soundlink 条目同步 | [desktop/src-tauri/Cargo.toml](../../../desktop/src-tauri/Cargo.toml) 等 |
| V4 CI 门 | `ci.yml` 增加 `version-check` job（setup-python 3.12 + `sync_version.py --check`） | [.github/workflows/ci.yml](../../../.github/workflows/ci.yml) |
| V5 Release 对齐 | `release.yml` 抽出 `version-gate` job（两个 build job `needs` 它）；NSIS 收集改用「找到且只有一个 setup.exe」断言 | [.github/workflows/release.yml](../../../.github/workflows/release.yml) |

## 关键设计决策

1. **VERSION 用纯文本而非 `version.json`**：任何语言/脚本/CI 都能一行读取，无解析依赖，diff 干净（plan §3.2）。
2. **同步目标排除 `website/package.json`**：官网是独立可部署站点，与客户端版本无耦合（plan §3.2）。
3. **pubspec 转换规则**：`<core 三段>+<BUILD_NUMBER>`，预发布后缀丢弃（Android `versionName` / iOS `CFBundleShortVersionString` 不接受 `-beta.1` 这类后缀；iOS 直接拒绝非纯数字点分）。`BUILD_NUMBER` 默认 `major*10000 + minor*100 + patch`，可用 `--build-number N` 覆盖（plan §3.3）。
4. **行级替换而非解析-重写**：目标字段都是单行，行级正则替换能保留原文件注释与格式；不引入 `toml` / `ruamel.yaml` 写库。Cargo.toml 限定 `[package]` 段内首个 `version = "..."`，避免误伤依赖项的 `version =`（plan §7 风险 1）。
5. **`--check` 模式只校验 versionName 部分**：移动端 build_number 允许 CI 单独覆盖为 `github.run_number`，不参与一致性校验；只校验 versionName（core 三段）必须等于 VERSION 的 core 三段。
6. **`version-gate` 抽成独立 job**：原计划在两个 build job 里各加校验步骤，会重复代码；改为顶层 `version-gate` job，desktop-windows 与 mobile-android 都 `needs: version-gate`，校验失败时跳过所有构建，节省 CI 时间。
7. **NSIS 产物收集用 `find ... | wc -l` 断言只有一个 `*-setup.exe`**：不依赖 Tauri 2 具体命名格式（`{productName}_{version}_{arch}-setup.exe`），只要求「有且仅有一个」，多于一个视为构建异常。比 `ls | head -n 1` 安全（避免静默取错）。

## 验证结果

```
$ .venv\Scripts\python.exe scripts\sync_version.py --check
[sync_version] VERSION = 0.1.0-beta.1  build_number = 100
[sync_version] 全部目标与 VERSION 一致。

$ cargo pkgid --offline   # 在 desktop/src-tauri
path+file:///.../desktop/src-tauri#soundlink@0.1.0-beta.1

$ python -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml')); yaml.safe_load(open('.github/workflows/ci.yml')); print('YAML OK')"
YAML OK

$ git diff --stat
 AGENTS.md                         | 1 +
 desktop/src-tauri/Cargo.toml      | 2 +-
 desktop/src-tauri/Cargo.lock      | 2 +-
 desktop/src-tauri/tauri.conf.json | 2 +-
 desktop/ui/package.json          | 2 +-
 mobile/flutter_app/pubspec.yaml  | 2 +-
 6 files changed, 7 insertions(+), 6 deletions(-)
```

`git diff --stat` 显示每个文件只改了 version 行，无格式破坏（pubspec.yaml 的空行保留——初版脚本 `\s*$` 跨行吞了空行，已修正为 `[ \t]*$`）。

## 用户需自行完成部分

1. **打首个 tag**（OSL-K3）：V1–V5 已解除 K3 前置阻塞。流程见 plan §6：
   ```powershell
   # 1. 编辑 VERSION（当前已是 0.1.0-beta.1，无需改）
   # 2. 同步到清单（如 VERSION 已改需重跑）
   .venv\Scripts\python.exe scripts\sync_version.py
   # 3. 本地验证门
   .venv\Scripts\python.exe scripts\sync_version.py --check
   # 4. 提交（用户本人执行，项目规则禁止代理提交/推送）
   # 5. 打 tag：tag 名必须 = v + VERSION 内容，否则 release.yml 的 version-gate 会 fail
   git tag v0.1.0-beta.1
   git push origin v0.1.0-beta.1
   # 6. push tag → CI 构建 → 核对 Draft Release → 手动 Publish
   ```
2. **后续发版流程**：每次改版本号只需改 `VERSION` + 跑 `sync_version.py`，不再手改 4 个清单。

## 第二轮：验收 + 版本意识体系（V12–V15，同日）

### 验收 V1–V5 的结论

实测通过（`--check` EXIT=0、4 个清单 + Cargo.lock 全为 `0.1.0-beta.1`、git diff 确认每个目标文件仅 1 行变更、格式与注释无破坏）。脚本中两处实现正好防住了计划 §7 预警的风险：`[package]` 段作用域限定（避免误伤依赖项 version）、`[ \t]*$` 行尾锚（避免 `\s*$` 跨行吞掉空行）。

但查出 3 处问题：

| # | 问题 | 处置 |
|---|---|---|
| 1 | **V3 标 `[x]` 但 P-6 未闭环**：脚本有 `--build-number`，`release.yml` 却没调用，`versionCode` 仍固定为 1 | 新增 V15；V3 备注改为「P-6 未闭环，转 V15」 |
| 2 | CHANGELOG 未回填 V1–V5，违反其自身回填规则第 1/3 条 | V14 |
| 3 | OSL 总览 §2 未同步，违反计划 §8 规则第 2 条 | V14 |

### 「AI 工作流有版本意识吗」——查证结果：没有

grep 证实：`CHANGELOG` 在 `AGENTS.md` 与 `.trae/rules/project-rules.md` 中出现 **0 次**；版本约束仅 `AGENTS.md` 常见任务表 1 行，且只讲「怎么改」不讲「何时改、改哪一位」。

**这是规则缺位而非执行失误**——直接证据就是上面的问题 2：代理把版本管理系统建完了，却没有任何规则提示它记录这件事。

### V12–V15 实现清单

| 任务 | 内容 | 文件 |
|---|---|---|
| V12 | 新建版本语义与判定规则文档：大小版本术语对照表（大版本=MAJOR / 小版本=MINOR / 修订=PATCH）、五级命中即停判定优先级、SoundLink 场景触发项、`0.x` 特殊规则、AI 义务 A–F、CHANGELOG 回填决策树、收尾自检 7 项 | [`01-versioning-policy.md`](../../../docs/NewFunctions/version-management/01-versioning-policy.md) |
| V13 | 「进度回填约束」升级为「进度与版本回填约束」，加入义务 A–F 表 + 01 文档指针；硬红线加「禁自行改 `VERSION`」，流程加 CHANGELOG 回填（全文 979 字符，符合 <1000 元规则） | [`AGENTS.md`](../../../AGENTS.md) / [`project-rules.md`](../../../.trae/rules/project-rules.md) |
| V14 | CHANGELOG 补「新增」2 条 +「变更」4 条；OSL §2 K 行 / K3 行 / K3 实操命令 / 发布前清单同步（tag 名修正 `v0.1.0-beta` → `v0.1.0-beta.1`，否则 `version-gate` 必 fail） | [`CHANGELOG.md`](../../../CHANGELOG.md) / [`00-launch-overview.md`](../../../docs/NewFunctions/opensource-launch/00-launch-overview.md) |
| V15 | `mobile-android` job 增加 setup-python + `sync_version.py --build-number ${{ github.run_number }}`，**置于 `flutter pub get` 之前** | [`release.yml`](../../../.github/workflows/release.yml) |

### 第二轮关键设计决策

1. **禁止代理自行 bump `VERSION`**（义务 C）：改 `VERSION` 等于宣布发版意图，属产品决策；且移动端 `versionCode` 不可回退，误 bump 有不可逆后果。代理只累积 `[未发布]` + 给出级别建议，发版动作归人类——与项目既有「禁 `git commit`」同源。
2. **「不升版本」≠「不用记录」**：两者独立判断。本次 V1–V15 自身不升版本，但必须写 CHANGELOG，这正是问题 2 的成因，故写入规则显式声明。
3. **`0.x` 阶段：版本号可宽松，告知不可以**（义务 F）：破坏性变更降级走 MINOR，但 CHANGELOG 条目必须带 ⚠ 与用户需执行的动作（如「需重新配对」「需卸载重装」）。
4. **Android 签名从 debug 切正式签名判为 MAJOR**：用户必须卸载重装，属破坏性变更，容易被误当 PATCH，故在 §3.2 显式列出。
5. **两份文档职责切分**：`00` 管「怎么改版本号」（机制/脚本/CI），`01` 管「何时改、改哪一位、谁来改」（语义/义务）。
6. **V13 是体系生效的关键**：没有规则文件挂钩，01 文档不会被后续会话读到，等于白写。
7. **文件名冲突修正**：计划 V7 原指向 `01-compatibility-matrix.md`，该名已被 policy 文档占用，改为 `02-compatibility-matrix.md`。

### 第二轮验证

- `--build-number 999` 实跑：pubspec 变为 `version: 0.1.0+999` ——确认 versionName 正确丢弃 `-beta.1` 预发布后缀（iOS 要求），仅 build_number 变化。
- 恢复默认后 `--check` EXIT=0，pubspec 回到 `0.1.0+100`。
- `project-rules.md` 字符数实测 979 < 1000。

## 已知边界

- **未做项**（V6–V11，建议/后续）：
  - V6 Release Notes 自动提取（CHANGELOG → Release body）
  - V7 兼容矩阵文档 / V8 协议版本单源化（`PROTOCOL_VERSION` 仍双端硬编码）
  - V9 移动端展示 App 版本 / V10 版本不匹配可读提示
  - V11 应用内更新检查（需先完成代码签名，签名前仅做「提示 + 跳转 Releases 页」）
- **iOS 版本号**：已核实 `ios/Runner/Info.plist` 用 `$(FLUTTER_BUILD_NAME)` / `$(FLUTTER_BUILD_NUMBER)`，随 pubspec 自动注入，无需额外同步目标。
- **Android `versionCode`**：首发 pubspec 写 `+1`（与 Flutter 模板默认等值，最小差异）；CI 用 `github.run_number` 覆盖保证单调递增（V15 已接线，见下文第二轮）。
- **行尾**：仓库无 `.gitattributes`，可能混用 CRLF/LF；脚本已稳健处理 `\r\n` 与 `\n`，不破坏原行尾。
- **iOS 商店后缀限制**：iOS 直接拒绝 `-beta.1` 这类非纯数字点分的 `CFBundleShortVersionString`，因此 pubspec 只写 core 三段；预发布信息通过 Release 页与 `BUILD_NUMBER` 体现。

## 关键文件索引

- 计划：[`docs/NewFunctions/version-management/00-version-management-plan.md`](../../../docs/NewFunctions/version-management/00-version-management-plan.md)
- 版本语义与 AI 义务：[`docs/NewFunctions/version-management/01-versioning-policy.md`](../../../docs/NewFunctions/version-management/01-versioning-policy.md)
- SSOT：[`VERSION`](../../../VERSION)
- 同步脚本：[`scripts/sync_version.py`](../../../scripts/sync_version.py)
- CI 门：[`.github/workflows/ci.yml`](../../../.github/workflows/ci.yml) `version-check` job
- Release 门：[`.github/workflows/release.yml`](../../../.github/workflows/release.yml) `version-gate` job
- 同步目标：[`desktop/src-tauri/Cargo.toml`](../../../desktop/src-tauri/Cargo.toml) / [`desktop/src-tauri/tauri.conf.json`](../../../desktop/src-tauri/tauri.conf.json) / [`desktop/ui/package.json`](../../../desktop/ui/package.json) / [`mobile/flutter_app/pubspec.yaml`](../../../mobile/flutter_app/pubspec.yaml)

## 关联文档

- 总览：[`../opensource-launch/00-launch-overview.md`](../opensource-launch/00-launch-overview.md)（阶段 K）
- 发布就绪度：[`../release-readiness/00-release-overview.md`](../release-readiness/00-release-overview.md) §4
- 计划/进度：[`../../First/12-plan.md`](../../First/12-plan.md)
- 前序会话：[FT-0015](./0015-2026-08-03-opensource-launch-audit.md)（开源发布审计，本会话为其后续）
