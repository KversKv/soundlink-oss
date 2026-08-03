<!-- OSL-00 -->
# 开源发布与社区运营规划总览

> 建档：2026-08-03 · 对象：整个仓库的**对外发布**工作（区别于 `release-readiness/` 的产品功能就绪度）
> 触发背景：Android → Windows、Windows → Windows 端到端已实测通过（2026-08-02），具备 `v0.1.0-beta` Early Access 条件。

---

## 1. 本目录与 `release-readiness/` 的分工

| 目录 | 回答的问题 | 关注点 |
|---|---|---|
| `release-readiness/` | **产品能不能发** | 功能完整度、安全红线、跨平台、测试 |
| `opensource-launch/`（本目录） | **怎么发、发给谁、发完做什么** | 仓库配套、Release 流水线、代码质量门槛、市场定位与推广 |

两者共用同一版本里程碑（`v0.1.0-beta` / `v0.2.0` / `v1.0.0`），见 [`../release-readiness/00-release-overview.md`](../release-readiness/00-release-overview.md) §4。

---

## 2. 阶段划分（OSL）

| 阶段 | 目标 | 优先级 | 状态 | 完成日期 |
|---|---|---|---|---|
| J · 仓库对外配套 | README / 许可 / 模板 / 安全策略 / 英文文档 | 必做 | ✅ 完成 | 2026-08-03 |
| K · Release 发布流水线 | tag 触发构建、产物上传、Release Notes、校验和 | 必做 | 🟡 进行中 | 2026-08-03（K1/K2 完成，K3-K5 待人工执行） |
| L · 代码质量对外门槛 | 全量 `cargo fmt`、CI 阻塞化、依赖审计 | 建议 | ⬜ 未开始 | — |
| M · 市场定位与社区运营 | 竞品调研、差异化文案、推广渠道、Issue 运营 | 建议 | 🟡 进行中 | 2026-08-03（M1 完成） |
| N · 产品官网 | 单页 landing（中英双语）+ GitHub Pages 部署，见 [`02-website-plan.md`](./02-website-plan.md) | 建议 | ⬜ 未开始 | 2026-08-03 完成规划，N1-N9 待实现 |

状态取值：`⬜ 未开始` / `🟡 进行中` / `✅ 完成` / `⏸ 暂停`。

---

## 3. 阶段 J · 仓库对外配套

| 任务 | 说明 | 状态 |
|---|---|---|
| J1 · LICENSE | MIT，署名 KversKv | [x] — 2026-07-12 |
| J2 · README 中文 | 问题陈述 / 功能矩阵 / 快速开始 / 已知限制 / 文档导航 | [x] — 2026-07-12 |
| J3 · CONTRIBUTING / CODE_OF_CONDUCT / SECURITY | 贡献流程、行为准则、漏洞私密上报 | [x] — 2026-07-12 |
| J4 · Issue / PR 模板 | `.github/ISSUE_TEMPLATE/{bug_report,feature_request,config}.yml` + `pull_request_template.md` | [x] — 2026-07-12 |
| J5 · CHANGELOG | Keep a Changelog 风格，`[未发布]` 收集中 | [x] — 2026-07-12 |
| J6 · 隐私政策 | `docs/privacy.md`，含第三方组件许可 | [x] — 2026-07-12 |
| J7 · 英文 README | `README.en.md`，与中文版结构对齐 | [x] — 2026-08-03 |
| J8 · 文档失效引用清零 | README / AGENTS / CHANGELOG / release-readiness 交叉引用可达 | [x] — 2026-08-03 |

> J7/J8 是上一轮遗漏项：CHANGELOG 与 README 曾提前声明 `README.en.md` 和本目录存在，但文件从未落地。本轮补齐。

---

## 4. 阶段 K · Release 发布流水线

| 任务 | 说明 | 状态 |
|---|---|---|
| K1 · CI 工作流 | `.github/workflows/ci.yml`：Rust clippy/test 矩阵 + 前端构建 + Flutter analyze/test | [x] — 2026-07-12 |
| K2 · Release 工作流 | `.github/workflows/release.yml`：`v*` tag 触发，构建 Windows exe/NSIS + Android APK，产出 SHA256，创建 Draft Release | [x] — 2026-08-03 |
| K3 · 首个 tag 与 Release | 打 `v0.1.0-beta`，核对 Draft 产物后发布为 Pre-release | [ ] 待人工执行 |
| K4 · Release Notes 模板 | 从 CHANGELOG `[未发布]` 提炼；必须含「仅实测 Android→Windows / Windows→Windows」「未签名会触发 SmartScreen」「DRM 不可采」三条免责 | [ ] |
| K5 · 产物可信度说明 | README/Release 页说明校验 SHA256 的方法；代码签名列为 `v1.0.0` 目标 | [ ] |

### K3 实操步骤（首发）

```powershell
# 1. 确认工作区干净、CHANGELOG [未发布] 内容已定稿
git status
# 2. 打 tag（beta 用 -beta 后缀，release.yml 会自动标记 prerelease）
git tag -a v0.1.0-beta -m "SoundLink v0.1.0-beta (Windows Early Access)"
git push origin v0.1.0-beta
# 3. 等 GitHub Actions 完成，到 Releases 页检查 Draft 产物与 Notes，再点 Publish
```

> 打 tag 与 push 由用户本人执行（项目规则禁止代理提交/推送）。

### 发布前必查清单

- [ ] `cargo test --features tauri_app`、`cargo clippy --features tauri_app -- -D warnings` 通过
- [ ] `npm run build`（`desktop/ui`）通过
- [ ] `flutter analyze` + `flutter test`（`mobile/flutter_app`）通过
- [ ] 用 Release 产物（而非本地 dev）重跑一次 Android → Windows 端到端
- [ ] CHANGELOG 把 `[未发布]` 改为 `[0.1.0-beta] - YYYY-MM-DD`
- [ ] `tauri.conf.json` 与 `pubspec.yaml` 版本号与 tag 一致
- [ ] Android APK 构建依赖的 libopus 源码获取方式已在文档说明（见 §5 L4）

---

## 5. 阶段 L · 代码质量对外门槛

| 任务 | 说明 | 状态 |
|---|---|---|
| L1 · 全量 `cargo fmt` | 存量代码未统一格式化，`ci.yml` 中 fmt 步骤当前 `continue-on-error`；完成后移除该标记 | [ ] |
| L2 · CI 阻塞化 | fmt/clippy 全部转为强制失败；给 `main` 加分支保护要求 CI 通过 | [ ] |
| L3 · 依赖审计 | 引入 `cargo audit`（或 Dependabot）+ `npm audit`，纳入 CI 周期任务 | [ ] |
| L4 · 第三方源码获取自动化 | `mobile/flutter_app/android/app/src/main/cpp/opus/` 被 gitignore 且未跟踪，外部贡献者与 CI 无法直接构建 Android。需二选一：脚本自动下载 libopus 源码，或改为 git submodule；并在 `docs/user/04-dev-env-android.md` 写明 | [ ] |

> L4 是**外部贡献者的首个卡点**，优先级高于 L1/L2。首发若来不及自动化，至少在 README/开发文档写明手动获取步骤。

---

## 6. 阶段 M · 市场定位与社区运营

| 任务 | 说明 | 状态 |
|---|---|---|
| M1 · 竞品调研与差异化定位 | 见 [`01-market-research.md`](./01-market-research.md) | [x] — 2026-08-03 |
| M2 · GitHub 仓库门面 | About 描述、Topics、社交预览图；README 顶部加 CI/License/Release badge | [ ] |
| M3 · 演示素材 | 一张 UI 截图 + 一段 15–30 秒操作录屏（配对 → 播放），放 README「使用方式」上方 | [ ] |
| M4 · 首发推广 | 渠道与文案见 `01-market-research.md` §5；发布节奏：先 GitHub Release → 再社区帖 | [ ] |
| M5 · 社区运营基线 | 开启 Discussions；定义 Issue 标签（`platform:windows/macos/linux/android/ios`、`type:bug/feature/compat`、`good first issue`）；承诺响应节奏 | [ ] |
| M6 · 反馈闭环 | 收集机型/路由器兼容性反馈，回填 `docs/user/08-troubleshooting.md` 与功能矩阵 | [ ] |

---

## 7. 当前主要风险

| 风险 | 影响 | 缓解 |
|---|---|---|
| 只有 Windows + Android 实测 | 用户按「跨平台」预期下载后失望，Issue 涌入 | README 功能矩阵已逐项标状态；Release 标 Pre-release 并在 Notes 首行写明实测组合 |
| 安装包未签名 | SmartScreen 告警，被误判为恶意软件 | Release Notes 明写原因 + 提供 SHA256；代码签名列入 v1.0.0 |
| Android 构建缺 libopus 源码 | 外部贡献者/CI 构建失败 | L4 |
| UI 仅中文 | 国际用户流失 | `README.en.md` 先行；UI i18n 见 release-readiness I3 |
| DRM / 部分应用不可采 | 用户认为是 Bug | README 已知限制 + Issue 模板中前置提示 |

---

## 8. 回填规则（强约束）

1. 完成任一任务后立即把 `[ ]` 改为 `[x]`，行末补 `— YYYY-MM-DD 备注`。
2. 阶段全部任务完成后，更新 §2 总表状态与完成日期，并同步 [`../release-readiness/00-release-overview.md`](../release-readiness/00-release-overview.md) §2 的 `🟢 OSL` 行。
3. 涉及项目阶段推进时，同步回填 `docs/First/12-plan.md`。
4. 验收未过不得标完成。

---

## 9. 关联文档

- 市场调研与定位：[`01-market-research.md`](./01-market-research.md)
- 产品官网规划：[`02-website-plan.md`](./02-website-plan.md)
- 产品发布就绪度：[`../release-readiness/00-release-overview.md`](../release-readiness/00-release-overview.md)
- 项目阶段进度：[`../../First/12-plan.md`](../../First/12-plan.md)
- 贡献与安全：[`../../../CONTRIBUTING.md`](../../../CONTRIBUTING.md)、[`../../../SECURITY.md`](../../../SECURITY.md)
- 会话归档：`docs/AI_Memory/FunctionTasks/`
