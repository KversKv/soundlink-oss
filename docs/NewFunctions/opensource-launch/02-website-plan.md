<!-- OSL-02 -->
# 产品官网设计与实现计划

> 建档：2026-08-03 · 阶段代号 **N** · 归属：`opensource-launch/`（对外发布配套）
> 用途：`v0.1.0-beta` 首发的官方落地页。承接 M2/M3/M4 的门面与素材需求，是 Release 页之外的第二落地点。
> 本文件只做**设计与计划**，不含实现代码。

---

## 1. 定位与决策

**一句话定位**：面向 GitHub 技术用户与中文效率工具用户的**单页产品落地页**，风格克制极简 + 深色技术感，唯一转化目标是「下载 Beta」。

已确认决策（2026-08-03）：

| 决策项 | 结论 | 理由 |
|---|---|---|
| 技术栈 | Vite + React 18 + TypeScript + Tailwind v4 | 与 `desktop/ui` 同栈，复用心智；纯静态产物 |
| 托管 | GitHub Pages（Actions 自动部署） | 零服务器成本，与仓库同源 |
| 页面范围 | 单页 landing，不做独立文档站 | 文档继续留在 `docs/user/`，页面外链过去 |
| 语言 | 中英双语，中文为默认 | 与 `README.md` / `README.en.md` 现状对齐；英文推广（Reddit/HN）已在路线图 |
| 目录位置 | 仓库根 `website/` | 与 Tauri 构建解耦，不污染 `desktop/` |

**不做的事**（首版明确排除）：无博客、无后端、无表单收集、无邮件订阅、无遥测/分析脚本（与产品「零遥测」承诺一致，若需流量数据只用 GitHub 自带 Insights）。

---

## 2. 视觉系统

| 维度 | 规格 |
|---|---|
| 主题 | **深色单一主题锁定**，全站不分区反色；`prefers-color-scheme` 不切换（品牌表达统一） |
| 底色 | 冷中性近黑 `#0B0C0E`，分区层次用 `#111316` / `#16181C` 微调 |
| 强调色 | **单色「信号青绿」`#2FD8A8`**，全站唯一强调色，用于主 CTA、状态「实测可用」、关键数字 |
| 禁用色 | 不用紫色/蓝色渐变光晕（AI 味），不用多强调色，不用外发光 |
| 字体 | 显示与正文 `Geist`；数字与规格 `Geist Mono`（延迟、码率、SHA256）。自托管 `@font-face` + `font-display: swap`，不外链 Google Fonts |
| 圆角 | 单一体系：卡片 `12px`、输入与徽标 `8px`、按钮 `8px`。不混 pill |
| 动效 | 强度低（`MOTION_INTENSITY: 4`）：Hero 入场淡入上移、分区滚动进入 stagger、CTA `:active` 下压 1px。仅动 `transform`/`opacity`，全部包 `prefers-reduced-motion` |
| 密度 | 疏（`VISUAL_DENSITY: 3`）：分区间距 `py-24` 到 `py-32` |
| 变化度 | 中（`DESIGN_VARIANCE: 6`）：Hero 非居中，分区布局家族不重复 |

**排版硬约束**：正文 `max-w-[65ch]`；标题最多 2 行；禁用衬线体；**全站禁用破折号 `—`/`–`**（用句号、逗号或普通连字符 `-`）。

---

## 3. 信息架构（单页分区）

排序即优先级。共 9 个分区，布局家族不得重复。

| # | 分区 | 内容要点 | 布局家族 |
|---|---|---|---|
| 1 | 导航 | Logo + 特性 / 原理 / 平台 / 文档 + 语言切换 + 主 CTA。桌面单行，高度 ≤ 72px，`<768px` 折叠汉堡 | Sticky bar |
| 2 | Hero | 标题：「把手机的声音，送到电脑的耳机上」。副文 ≤ 20 字词、≤ 3 行。主 CTA「下载 Beta」+ 次 CTA「查看源码」。右侧放**真实桌面端截图** | 非对称左右分栏（左文右图） |
| 3 | 平台支持矩阵 | Windows / Android / macOS / Linux / iOS 图标 + 角色 + 诚实状态徽标（✅ 实测 / 🟡 未实测 / 🔴 未实装）。**紧贴 Hero 下方，不塞进 Hero** | 图标状态栅格（等宽单行，代替常规 logo wall） |
| 4 | 场景陈述 | 问题描述 + 适用/不适用（听音乐看视频 ✅，游戏连麦 ❌）。配一张生活场景图 | 全宽单栏 editorial + 背景图 |
| 5 | 三条差异化 | ① 免开发者模式（不像 sndcpy/scrcpy 要 USB 调试）② 默认加密（ChaCha20-Poly1305，密钥进系统钥匙串，零遥测）③ 真开源免费（MIT，无订阅无广告）| **非对称 bento（1 大 + 2 小，共 3 格）**，其中至少 2 格含真实图/图案 |
| 6 | 工作原理 | 三步实拍：桌面开启接收并显示 8 位配对码 → 手机输入配对码 → 播放即出声。标题用动词（「开启接收」/「输入配对码」/「开始播放」），**禁用「步骤 1/2/3」标签** | 三栏截图序列（带连接线） |
| 7 | 技术规格 | 分 3 组，不做逐行 hairline 表：**音频**（48 kHz / Stereo / Opus 10 ms / 128 kbps / Jitter 80 ms）·**安全**（X25519 + Ed25519 + ChaCha20-Poly1305 / OS keyring / 零遥测）·**传输**（音频 UDP + 控制 TCP，仅局域网）。数字用 mono | 三组规格卡（每组一条软分隔） |
| 8 | 已知限制 | 直接搬 README「已知限制」：仅局域网、DRM 不可采、单接收端、无 USB 模式、延迟不适合游戏、桌面 UI 仅中文、安装包未签名。**诚实即卖点，不藏折叠里** | 两列分组清单 |
| 9 | 下载 | 指向 GitHub Release（Pre-release）。含 SHA256 校验方法 + SmartScreen 告警原因说明 + 「仅实测 Android→Windows / Windows→Windows」提示 | 全宽居中 CTA 区 |
| 10 | 页脚 | MIT 许可 / 隐私政策 / 用户文档 / CHANGELOG / 仓库 / 安全上报 | 多列链接 |

**文案纪律**：
- 转化意图只有一个标签「下载 Beta」（导航、Hero、下载区三处**用词完全一致**）；次要 CTA 统一为「查看源码」。
- **小号大写 eyebrow 标签全站最多 2 处**（Hero 算 1 处），其余分区靠标题本身承担。
- 禁止：滚动提示（`↓ 向下滚动`）、版本号装饰（`v0.6` / `BETA` 徽章当装饰）、分区编号（`01 / 特性`）、城市时间天气条、div 拼的假界面截图、装饰性状态圆点、`·` 当万能分隔符（每行最多 1 个）。
- 延迟措辞：只说「目标 100 ms 级 / 默认 Jitter 80 ms」，**不编造未实测的精确数字**。

---

## 4. 素材清单（实现的前置阻塞项）

网站不能靠纯文字撑，以下素材缺失则对应分区无法完成。与 M3 共用。

| 编号 | 素材 | 用途 | 规格 |
|---|---|---|---|
| A1 | 桌面端主界面截图（深色，接收中 + 配对码状态） | Hero 主视觉 | ≥ 2400×1500，PNG |
| A2 | 手机端配对界面截图 | 分区 6 第二步 | 手机原始比例 |
| A3 | 桌面端「设备已连接 / 播放中」截图 | 分区 6 第三步 | 同 A1 |
| A4 | 场景图（耳机 + 桌面 + 手机） | 分区 4 背景 | 横向 ≥ 2400px；缺素材时用 `picsum.photos/seed/...` 占位并标注 TODO |
| A5 | 平台图标（Windows/Android/macOS/Linux/Apple） | 分区 3 | Simple Icons CDN 或 npm `simple-icons`，禁手写 SVG 路径 |
| A6 | SoundLink 品牌标记 | 导航 + favicon + OG 图 | 简单几何字标即可，与桌面端图标一致 |
| A7 | 15-30 秒操作录屏 | 可选，嵌在分区 6；同时复用到 README 与社区帖 | MP4/WebM，无音轨 |
| A8 | 社交预览图（OG/Twitter card） | 分享卡片 + M2 | 1200×630 |

图标库统一 `@phosphor-icons/react`，`strokeWidth` 全站固定，不混用第二套。

---

## 5. 工程方案

```
website/
├── index.html              # 中文入口（默认）
├── en/index.html           # 英文入口（Vite 多页构建，共享组件）
├── public/                 # 截图、字体、favicon、og 图、CNAME（如启用自定义域）
├── src/
│   ├── content/{zh,en}.ts  # 全部文案集中在此，组件不写死中文
│   ├── sections/           # 每个分区一个组件，与 §3 表格一一对应
│   ├── components/         # Button / Badge / PlatformStatus / SectionShell
│   └── styles/theme.css    # Tailwind v4 + 设计令牌（色/圆角/间距）
└── vite.config.ts          # base 按 Pages 路径配置；@tailwindcss/vite 插件
```

要点：
- **双语用多页而非 SPA 切换**，保证 SEO 与 `hreflang` 正确；语言切换是两个入口间的链接，附 `localStorage` 记住选择 + 首访按 `navigator.language` 建议跳转（不强制）。
- 文案 `zh.ts` / `en.ts` **键名必须完全一致**，缺键在构建期由类型检查暴露。
- 与产品仓库的**单源约束**：音频基线、加密算法、平台矩阵、已知限制这四组内容以 `README.md` 与 `docs/First/11-implementation-spec.md` 为准；网站修改这些数字时必须同步核对，禁止各写一套。
- 下载链接指向 `releases/latest`，不硬编码版本号。
- 部署：新增 `.github/workflows/pages.yml`，`main` 分支 `website/**` 变更时构建并发布到 Pages；与现有 `ci.yml` / `release.yml` 互不影响。

---

## 6. 实现计划（阶段 N）

| 任务 | 说明 | 依赖 | 状态 |
|---|---|---|---|
| N1 · 脚手架 | `website/` 初始化 Vite + React + TS + Tailwind v4；多页入口；设计令牌落地 | 无 | [x] — 2026-08-04 完成，构建通过 |
| N2 · 内容层 | `content/zh.ts` + `en.ts` 全量文案定稿（含 §3 所有分区、免责声明） | 无 | [x] — 2026-08-04 键名经类型对齐 |
| N3 · 素材采集 | A1-A6 截图与图标就位（A7/A8 可延后） | 需运行桌面端与 Android 端 | [ ] — 代码已留 TODO 占位（`public/assets/placeholder.svg`），待真实截图替换；字体文件待放入 `public/fonts/` |
| N4 · 分区实现 | 按 §3 顺序实现 10 个分区组件 + 移动端单列折叠 | N1-N3 | [x] — 2026-08-04 10 分区全部实现（素材用占位图） |
| N5 · 动效层 | 入场与滚动进入动效 + `prefers-reduced-motion` 降级 | N4 | [x] — 2026-08-04 fade-up + IntersectionObserver reveal + reduced-motion 静态化 |
| N6 · SEO 与元信息 | title/description/OG/Twitter card/`hreflang`/`sitemap.xml`/`robots.txt` | N2、A8 | [x] — 2026-08-04 已落地（OG 图 `og.png` 待补，A8） |
| N7 · 部署流水线 | `.github/workflows/pages.yml` + 仓库 Pages 设置（需用户在 GitHub 后台开启） | N1 | [x] — 2026-08-04 workflow 已建；**需用户在仓库 Settings → Pages 选择 GitHub Actions 源** |
| N8 · 验收 | 按 §7 清单逐项过 | N4-N7 | [ ] — 待真实素材与部署后实地验收 |
| N9 · 交叉引用 | README 顶部加官网链接；`00-launch-overview.md` §2 回填 N 行状态；M2/M3 标注素材复用 | N8 | [ ] — 待 N8 通过后执行 |

**回填约束**：完成任一任务即把 `[ ]` 改为 `[x]` 并补 `— YYYY-MM-DD 备注`；阶段完成后同步 [`00-launch-overview.md`](./00-launch-overview.md) §2 与 `docs/First/12-plan.md`。验收未过不得标完成。

---

## 7. 验收标准

**性能与可访问性**
- [ ] Lighthouse 移动端：Performance ≥ 90，Accessibility ≥ 95，Best Practices ≥ 95，SEO ≥ 95
- [ ] LCP < 2.5s（Hero 截图 `preload` + 提供 WebP/AVIF + 显式宽高防 CLS）、CLS < 0.1
- [ ] 全部文字对比度过 WCAG AA（正文 4.5:1）；CTA 文字在按钮底色上可读
- [ ] 键盘可完整导航，焦点环可见；图片有 `alt`
- [ ] `prefers-reduced-motion: reduce` 下动效全部静态化

**布局与内容**
- [ ] Hero 在 1440×900 与 1280×720 下完整可见，CTA 无需滚动即出现；标题 ≤ 2 行；`min-h-[100dvh]` 而非 `h-screen`
- [ ] 导航桌面单行、高度 ≤ 72px
- [ ] 9 个分区布局家族无重复；无 3 段以上连续「左图右文」交替
- [ ] eyebrow 标签总数 ≤ 2
- [ ] 全站零 `—`/`–`
- [ ] 中英文案键完全对齐，无残留占位符与未翻译串
- [ ] 无 div 伪造的界面截图；无手写装饰 SVG
- [ ] 平台状态与 `README.md` 功能矩阵逐行一致；音频基线数字与规格文档一致
- [ ] 免责三条齐全（实测组合范围 / 未签名会触发 SmartScreen / DRM 不可采）

**移动端**：360px 宽度下无横向滚动，所有多列布局在 `<768px` 折叠为单列。

---

## 8. 风险与缓解

| 风险 | 影响 | 缓解 |
|---|---|---|
| 截图素材不足，页面沦为纯文字 | 落地页说服力归零 | N3 列为硬依赖，未就位不得进入 N4；实在缺则用标注清楚的占位图并在交付说明中列出待补位置 |
| 官网口径与 README 漂移 | 用户预期错位，Issue 涌入 | §5「单源约束」；N9 交叉引用；每次改数字先核对 README |
| 过度承诺跨平台 | 与 OSL §7 同一风险放大 | 平台矩阵用三色状态徽标，`🔴/🟡` 不加下载按钮 |
| Pages 部署路径导致资源 404 | 站点白屏 | `vite.config.ts` 的 `base` 与 Pages 路径（子路径或自定义域）一次性确认，N7 部署后实地验证 |
| 引入分析脚本破坏「零遥测」叙事 | 信任受损 | 明确不引入任何第三方脚本 |

---

## 9. 关联文档

- 发布总览与阶段表：[`00-launch-overview.md`](./00-launch-overview.md)
- 定位与卖点文案来源：[`01-market-research.md`](./01-market-research.md) §3、§4
- 平台矩阵与已知限制来源：[`../../../README.md`](../../../README.md)
- 音频与协议基线：[`../../First/11-implementation-spec.md`](../../First/11-implementation-spec.md)
- 安全模型：[`../../../SECURITY.md`](../../../SECURITY.md)、[`../../First/05-pairing-security.md`](../../First/05-pairing-security.md)
- 用户文档（页面外链目标）：[`../../user/00-index.md`](../../user/00-index.md)
