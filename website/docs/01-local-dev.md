<!-- WEB-01 -->
# 官网本地调试指南

> 适用于 `website/`（SoundLink 产品官网）。技术栈：Vite 5 + React 18 + TypeScript 5 + Tailwind CSS v4。
> 设计与验收依据：[`docs/NewFunctions/opensource-launch/02-website-plan.md`](../../docs/NewFunctions/opensource-launch/02-website-plan.md)

## 1. 环境要求

| 项 | 要求 | 说明 |
|---|---|---|
| Node.js | **20 LTS** | 与 CI（`.github/workflows/pages.yml` 中 `node-version: 20`）保持一致 |
| 包管理器 | npm（随 Node 附带） | 仓库提供 `package-lock.json`，勿混用 pnpm/yarn |
| 终端 | PowerShell（Windows） | 本文命令均以 PowerShell 为准 |

检查版本：

```powershell
node -v   # 期望 v20.x
npm -v
```

## 2. 首次准备

`website/` 的依赖**不随仓库提交**，克隆后必须先安装，否则 `npm run build` 会报 `'tsc' 不是内部或外部命令`。

```powershell
cd d:\CodeProject\TRAE_Projects\SoundLink\website
npm install
```

## 3. 常用命令

| 命令 | 作用 | 输出 |
|---|---|---|
| `npm run dev` | 启动开发服务器（HMR） | `http://localhost:5173/SoundLink/` |
| `npm run build` | `tsc -b` 类型检查 + Vite 构建 | `website/dist/` |
| `npm run preview` | 以静态服务器预览 `dist/`（最接近线上） | `http://localhost:4173/SoundLink/` |

发布前的最小自检顺序：`npm run build` → `npm run preview` → 浏览器验证两个语言入口。

## 4. 访问地址（重要）

`vite.config.ts` 设置了 `base: '/SoundLink/'`（GitHub Pages 项目页子路径），因此**本地地址也带 `/SoundLink/` 前缀**：

| 页面 | 本地地址 |
|---|---|
| 中文 | `http://localhost:5173/SoundLink/` |
| 英文 | `http://localhost:5173/SoundLink/en/` |

访问 `http://localhost:5173/` 会得到 404，这是预期行为，不是 bug。

## 5. 双语与语言跳转陷阱

站点为**多页**结构（非 SPA 切语言）：

- 入口：`index.html` → `src/main.tsx` → `content/zh.ts`
- 入口：`en/index.html` → `src/main-en.tsx` → `content/en.ts`
- 两份文案文件的**键名必须完全一致**，`en.ts` 通过 `Content = typeof zh` 做类型约束；缺键会在 `npm run build` 的 `tsc -b` 阶段直接报错。

两个 HTML 的 `<head>` 内有一段语言重定向脚本，读写 `localStorage` 的 `soundlink-lang` 键：

- 值为 `en` → 中文页自动跳到 `/SoundLink/en/`
- 值为 `zh` → 英文页自动跳回 `/SoundLink/`
- 无值且浏览器语言非 `zh*` → 写入 `en` 并跳转

**调试现象**：一旦点过语言切换按钮，之后打开中文页会被立刻弹到英文页，看起来像"中文页坏了"。

清除方式（浏览器 DevTools → Console）：

```js
localStorage.removeItem('soundlink-lang')
```

或使用无痕窗口调试。注意该脚本内的跳转路径是**硬编码 `/SoundLink/`**，与 `vite.config.ts` 的 `base` 是两处独立配置，改 `base` 时必须同步改这两个 HTML。

## 6. 目录职责

```text
website/
├─ index.html            中文入口（含 hreflang / OG / 语言跳转脚本）
├─ en/index.html         英文入口
├─ vite.config.ts        base 与多页 rollupOptions.input
├─ public/               原样复制到 dist 根（favicon / robots / sitemap / assets）
└─ src/
   ├─ main.tsx           注入 zh 文案
   ├─ main-en.tsx        注入 en 文案
   ├─ App.tsx            按规划 §3 顺序组装 10 个分区
   ├─ content/{zh,en}.ts 全站文案单源
   ├─ sections/          10 个分区组件
   ├─ components/        Button / SectionShell / StatusBadge
   └─ styles/theme.css   Tailwind v4 @theme 设计令牌 + 动效
```

改文案只动 `content/`，改结构只动 `sections/`，改配色圆角只动 `theme.css` 的 `@theme` 块。

## 7. 设计约束自检（提交前）

来自规划 §2/§3 的硬约束，可用命令快速核验：

```powershell
# 全站零破折号（应无输出）
Select-String -Path .\src\**\*.ts*,.\index.html,.\en\index.html -Pattern '[—–]'

# eyebrow 小标签数量（上限 2 处）
Select-String -Path .\src\**\*.tsx -Pattern 'uppercase tracking'
```

其余人工检查项：单一 CTA 用词「下载 Beta」、圆角只用 `rounded-[12px]`/`rounded-[8px]`、强调色只用 `text-accent`/`bg-accent`、动效只改 `transform`/`opacity`。

## 8. 已知待办与预期告警

以下现象**属当前已知未完成项**，不是环境问题：

| 现象 | 原因 | 处理 |
|---|---|---|
| 构建输出 `/fonts/geist-sans.woff2 ... didn't resolve at build time` ×2 | `public/fonts/` 目录不存在，Geist 字体未放入 | 放入 `geist-sans.woff2` / `geist-mono.woff2`；同时注意 `theme.css` 中路径为绝对 `/fonts/...`，未走 `base`，线上会 404，需改为 `/SoundLink/fonts/...` 或相对路径 |
| 页面字体是系统默认（非 Geist） | 同上，`font-display: swap` 回退 | 同上 |
| 截图区域显示灰色占位图 | A1/A2/A3 素材未采集，代码内为 `TODO(A1..A3)` + `public/assets/placeholder.svg` | 采集真实截图后替换 `content/*.ts` 的 `screenshotSrc` 等字段 |
| 场景图来自外部域名 | `Scenario.tsx` 暂用 `picsum.photos` 外链（`TODO(A4)`） | 替换为本地 `public/assets/` 图片；官网承诺零第三方请求，上线前必须换掉 |
| 分享卡片无图 | `og.png` 未生成（A8），但两个 HTML 已引用 | 出 1200×630 图放 `public/og.png` |

## 9. 排障速查

| 症状 | 处理 |
|---|---|
| `'tsc' 不是内部或外部命令` | 未装依赖，执行 `npm install` |
| `npm run dev` 打开是 404 | 地址漏了 `/SoundLink/` 前缀 |
| 中文页秒跳英文页 | 清除 `localStorage` 的 `soundlink-lang` |
| 改了 `zh.ts` 但英文页报类型错误 | 两份文案键名不一致，补齐 `en.ts` |
| 5173 端口被占用 | Vite 自动改用 5174，看终端实际输出的地址 |
| `preview` 页面白屏、控制台 404 资源 | `base` 与访问路径不匹配，确认从 `/SoundLink/` 进入 |

## 10. 关联文档

- 部署：[`02-deploy-pages.md`](./02-deploy-pages.md)
- 设计与验收规格：[`02-website-plan.md`](../../docs/NewFunctions/opensource-launch/02-website-plan.md)
- 发布总览：[`00-launch-overview.md`](../../docs/NewFunctions/opensource-launch/00-launch-overview.md)
