<!-- FT-0015 -->
# 开源发布筹备评估与文档补齐（2026-08-03）

> 场景：Android → Windows / Windows → Windows 端到端实测通过后，用户要求「评估上一轮未完成的 GitHub 发布筹备、做市场调研、梳理优化全项目文档、汇总开源测试还需做什么」。

---

## 1. 背景

上一轮（2026-07-12）已完成开源仓库大部分配套（LICENSE / CONTRIBUTING / SECURITY / CODE_OF_CONDUCT / CHANGELOG / Issue-PR 模板 / CI 工作流 / README 重写），但**任务中断**，留下一批「文档声明了但文件不存在」的悬空引用。

---

## 2. 评估结论：上一轮的 6 项残留

| # | 问题 | 证据 | 本轮处理 |
|---|---|---|---|
| 1 | `README.en.md` 被引用但不存在 | [README.md:7](file:///d:/CodeProject/TRAE_Projects/SoundLink/README.md#L7)、CHANGELOG.md:10 | ✅ 已补齐文件 |
| 2 | `docs/NewFunctions/opensource-launch/` 整个目录被引用但不存在（6 处引用） | README.md:150/167、AGENTS.md:51、CHANGELOG.md:11、release-readiness/00:22/33 | ✅ 已新建总览 + 市场调研 |
| 3 | CHANGELOG `[未发布]` 含虚假完成声明 | 上述两项 | ✅ 文件补齐后声明成立，并补记 release.yml |
| 4 | CI 已实装但文档仍写「无 CI」 | `03-p2-future-optimizations.md` H6、`00-release-overview.md` §1 | ✅ H6 勾选并写明现状与遗留 |
| 5 | 无 `release.yml`，`git tag` 为空，首个 Release 未发 | `.github/workflows/` 仅 ci.yml | ✅ 已新增 release.yml；打 tag 待用户执行 |
| 6 | Android 构建依赖的 libopus 源码未纳入版本管理 | `mobile/flutter_app/android/.gitignore:18` 忽略 `cpp/opus/`，`git ls-files` 0 条，无 `.gitmodules` | 🟡 release.yml 内已加自动下载步骤；仓库侧治理列为 OSL-L4 |

> 结论：上一轮**没有代码缺陷，只有文档与流水线的断点**。产品侧（P0/P1）确实已完成，具备 `v0.1.0-beta` Windows Early Access 条件。

---

## 3. 市场调研要点

调研 6 个竞品：AudioRelay、SoundWire、sndcpy、scrcpy v4.x、AudioShare、Shairport4w。完整对比表见 [`01-market-research.md`](../../NewFunctions/opensource-launch/01-market-research.md)。

核心发现：

1. **方向错位即机会**：SoundWire / AudioShare 做的是「电脑 → 手机」，与本项目场景相反；真做「手机 → 电脑」的只有 AudioRelay（专有收费）与 sndcpy/scrcpy（必须开 USB 调试）。
2. **零前置条件是护城河**：SoundLink 不需要 ADB / USB 调试 / root / 虚拟声卡。
3. **加密是行业空白**：调研范围内无竞品宣称音频面加密；SoundLink 有 ChaCha20-Poly1305 + OS keyring + 零遥测。
4. **iOS 是全行业缺口**：AudioRelay / SoundWire 均无 iOS 发送端，现有替代只有 AirPlay。→ 因此把 **iOS 真机验收列为市场优先级最高**的功能缺口。
5. **必须诚实披露的劣势**：无 USB 模式、单接收端、无麦克风回传、UI 仅中文、未签名、仅两组合实测。

---

## 4. 实现清单

| 类型 | 文件 | 说明 |
|---|---|---|
| 新增 | [docs/NewFunctions/opensource-launch/00-launch-overview.md](file:///d:/CodeProject/TRAE_Projects/SoundLink/docs/NewFunctions/opensource-launch/00-launch-overview.md) | OSL 阶段 J/K/L/M 总表、K3 打 tag 实操步骤、发布前必查清单、风险表、回填规则 |
| 新增 | [docs/NewFunctions/opensource-launch/01-market-research.md](file:///d:/CodeProject/TRAE_Projects/SoundLink/docs/NewFunctions/opensource-launch/01-market-research.md) | 竞品对比表、优劣势、GitHub 文案卖点与 Topics、推广渠道与节奏、功能路线市场优先级 |
| 新增 | [README.en.md](file:///d:/CodeProject/TRAE_Projects/SoundLink/README.en.md) | 与中文 README 结构对齐，含功能矩阵、构建、限制、文档索引 |
| 新增 | [.github/workflows/release.yml](file:///d:/CodeProject/TRAE_Projects/SoundLink/.github/workflows/release.yml) | `v*` tag 触发；Windows 构建 portable exe + NSIS；Android 构建 split-per-abi APK（含 libopus 自动下载）；产出 SHA256；创建 Draft Release 并预填免责说明 |
| 修改 | [README.md](file:///d:/CodeProject/TRAE_Projects/SoundLink/README.md) | 文档导航加市场调研入口；已知限制补「UI 仅中文」「未签名」；当前状态纠正 CI 描述 |
| 修改 | [AGENTS.md](file:///d:/CodeProject/TRAE_Projects/SoundLink/AGENTS.md) | P2 清单把 H6 移入已完成；OSL 阶段状态细化 |
| 修改 | [CHANGELOG.md](file:///d:/CodeProject/TRAE_Projects/SoundLink/CHANGELOG.md) | 补 release.yml 条目，细化 opensource-launch 描述 |
| 修改 | [release-readiness/00-release-overview.md](file:///d:/CodeProject/TRAE_Projects/SoundLink/docs/NewFunctions/release-readiness/00-release-overview.md) | §1 缺口去掉「无 CI」、§3 H 行、§4 v0.2.0 范围去掉 CI |
| 修改 | [release-readiness/03-p2-future-optimizations.md](file:///d:/CodeProject/TRAE_Projects/SoundLink/docs/NewFunctions/release-readiness/03-p2-future-optimizations.md) | H6 勾选，写明 3 个 job 现状与 fmt 阻塞化遗留 |
| 修改 | [docs/First/12-plan.md](file:///d:/CodeProject/TRAE_Projects/SoundLink/docs/First/12-plan.md) | §1 总表后增「发布侧进度」指针块 |

---

## 5. 关键设计决策

1. **两个 NewFunctions 子目录职责切分**：`release-readiness/` 回答「产品能不能发」，`opensource-launch/` 回答「怎么发、发给谁、发完做什么」。共用同一版本里程碑，避免双份真相源。
2. **OSL 阶段编号沿用上一轮埋点**：`ci.yml` 注释中已出现 `OSL-L1`（全量 `cargo fmt` 后移除 `continue-on-error`），本轮总览严格对齐该编号，不重新编号。
3. **release.yml 只出 Draft**：产物名带版本与平台、附 SHA256，Release Notes 预填「实测范围 / 未签名 / DRM / 仅中文」四类免责，人工核对后再 Publish。打 tag 与 push 交由用户执行（项目规则禁止代理提交推送）。
4. **libopus 在 CI 内下载而非入库**：保持仓库不含第三方源码，同时让 CI 可构建；仓库侧长期方案（脚本或 submodule）记为 OSL-L4，并标注它是外部贡献者的首个卡点。
5. **Android APK 用 debug 签名的事实写进 Release Notes**：`android/app/build.gradle.kts:48` 目前 release 走 debug 签名，必须提前告知用户「与后续正式签名版不可覆盖升级」。
6. **文档对外只写实测结论**：功能矩阵与 Release Notes 均逐项标注状态，避免用户按「跨平台」预期下载后涌入 Issue。

---

## 6. 开源发布还需要做什么（交付给用户的待办）

**必做（发首个 Release 前）**

1. 本地跑一遍验证：`cargo test/clippy --features tauri_app`、`desktop/ui` 的 `npm run build`、`flutter analyze` + `flutter test`。
2. CHANGELOG 把 `[未发布]` 定稿为 `[0.1.0-beta] - YYYY-MM-DD`。
3. 打 tag 并推送：`git tag -a v0.1.0-beta -m "..."` + `git push origin v0.1.0-beta`（OSL-K3）。
4. 到 Releases 页核对 Draft 产物与 Notes，改为 Pre-release 后 Publish。
5. 用 Release 产物本身（而非 dev 构建）重跑一次 Android → Windows 端到端。

**强烈建议（同期）**

6. GitHub 仓库门面：About 一句话、Topics、社交预览图、README 顶部 badge（OSL-M2）。
7. 一张 UI 截图 + 15–30 秒操作录屏放进 README（OSL-M3）——这是开源项目转化率最高的单项投入。
8. 开启 Discussions，建 Issue 标签体系（OSL-M5）。
9. libopus 获取自动化或至少写清手动步骤（OSL-L4）。

**后续（英文推广前）**

10. UI 英文 i18n（release-readiness I3）。
11. 全量 `cargo fmt` 并把 CI fmt 转为阻塞（OSL-L1/L2）。
12. iOS 真机验收（市场优先级最高的功能缺口）。
13. macOS 接收端实测（G3）。

---

## 7. 已知边界

- 竞品延迟数字均为对方宣称或社区反馈，非统一环境实测，文档中已注明仅作量级参考。
- `release.yml` 未在 GitHub Actions 上实跑（本地无法验证 runner 行为）；首次打 tag 时需关注三处高风险点：NSIS 产物通配路径、Flutter Android 构建的 NDK/CMake 可用性、libopus 下载源可达性。
- 未改动任何产品代码，音频链路与协议零变更。

---

## 8. 关联文档

- 开源发布总览：`docs/NewFunctions/opensource-launch/00-launch-overview.md`
- 市场调研：`docs/NewFunctions/opensource-launch/01-market-research.md`
- 产品就绪度：`docs/NewFunctions/release-readiness/00-release-overview.md`
- 上一轮 P2 批次实录：[FT-0014](./0014-2026-07-12-p2-tests-and-ux-batch.md)
