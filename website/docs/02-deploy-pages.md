<!-- WEB-02 -->
# 静态网页部署指南（GitHub Pages）

> 适用于 `website/`。托管方案：GitHub Actions 构建 + GitHub Pages 发布，无服务器、无后端。
> 流水线文件：[`.github/workflows/pages.yml`](../../.github/workflows/pages.yml)

## 1. 部署形态

| 项 | 当前取值 |
|---|---|
| 托管 | GitHub Pages（项目页） |
| 线上地址 | `https://kverskv.github.io/SoundLink/` |
| 构建产物 | `website/dist/`（不提交仓库，由 CI 生成） |
| Vite `base` | `/SoundLink/` |
| 发布方式 | `actions/upload-pages-artifact@v3` + `actions/deploy-pages@v4` |

## 2. 一次性设置（首次部署必做）

1. 打开仓库 **Settings → Pages**。
2. **Build and deployment → Source** 选择 **GitHub Actions**（不要选 `Deploy from a branch`）。
3. 保存后回到 **Actions** 页确认 `Deploy Website to Pages` 工作流可见。

未完成这一步时，工作流的 `deploy` job 会失败，报环境或权限相关错误。

## 3. 自动部署触发条件

`pages.yml` 的触发规则：

```yaml
on:
  push:
    branches: [main]
    paths: ['website/**', '.github/workflows/pages.yml']
  workflow_dispatch:
```

- 只有 `website/**` 或工作流本身发生变更并推到 `main` 才会构建，改 Rust/移动端代码不会触发。
- 需要手动重跑时：**Actions → Deploy Website to Pages → Run workflow**。
- `concurrency: { group: pages, cancel-in-progress: false }` 保证部署串行，不会互相打断。

流水线两个 job：

| Job | 步骤 |
|---|---|
| `build` | checkout → setup-node 20（缓存 `website/package-lock.json`）→ `npm ci` → `npm run build` → 上传 `website/dist` 为 Pages artifact |
| `deploy` | 消费 artifact，调用 `deploy-pages@v4` 发布到 `github-pages` 环境 |

## 4. 部署前本地验证

CI 与本地用同一套命令，建议推送前先在本地跑通：

```powershell
cd D:\CodeProject\TRAE_Projects\SoundLink\oss\website
npm ci
npm run build
npm run preview
```

`npm run preview` 起在 `http://localhost:4173/SoundLink/`，路径结构与线上一致，是验证 `base` 是否正确的最可靠方式。

需要确认的产物结构：

```text
website/dist/
├─ index.html          中文页
├─ en/index.html       英文页
├─ assets/*.js|*.css   带 hash 的构建资源
├─ favicon.svg
├─ robots.txt
└─ sitemap.xml
```

若 `en/index.html` 缺失，说明 `vite.config.ts` 的 `rollupOptions.input` 多入口配置被破坏。

## 5. 路径与域名（最易出错的一环）

站内存在**四处**与站点地址耦合的配置，切换部署形态时必须同步修改，否则会出现资源 404 或 SEO 指向错误：

| 位置 | 内容 | 项目页 `/SoundLink/` | 自定义域根路径 |
|---|---|---|---|
| `vite.config.ts` | `base` | `'/SoundLink/'` | `'/'` |
| `index.html` / `en/index.html` | `hreflang` 三条 + `og:image` | `/SoundLink/...` | `/...` 或完整域名 |
| `index.html` / `en/index.html` | 语言跳转脚本内的 `location.replace()` | `/SoundLink/` `/SoundLink/en/` | `/` `/en/` |
| `public/robots.txt` / `public/sitemap.xml` | 绝对 URL | `https://kverskv.github.io/SoundLink/` | 新域名 |

组件内引用的静态资源统一走 `import.meta.env.BASE_URL`（见 `content/zh.ts` 的 `ASSET()` 与 `Nav.tsx` 的 favicon），改 `base` 后自动跟随，无需逐个改。

**例外**：`src/styles/theme.css` 的 `@font-face` 写死了 `/fonts/geist-*.woff2`，CSS 中无法使用 `import.meta.env`，在 `base: '/SoundLink/'` 下线上会 404。放入字体文件时需同时把路径改成 `/SoundLink/fonts/geist-sans.woff2`（或相对路径）。

### 启用自定义域

1. 在 `website/public/` 新增 `CNAME` 文件，内容为裸域名（如 `soundlink.dev`），Vite 会原样复制到 `dist/`。
2. 按上表把 `base` 改为 `'/'` 并同步其余三处。
3. DNS 配置 `A`/`CNAME` 记录指向 GitHub Pages，仓库 Settings → Pages 填入域名并勾选 **Enforce HTTPS**。

## 6. 部署后验收

| 检查项 | 方法 |
|---|---|
| 两个语言入口可达 | 访问 `/SoundLink/` 与 `/SoundLink/en/` |
| 语言切换往返正常 | 点 `EN` / `中文`，观察 URL 与 `localStorage.soundlink-lang` |
| 静态资源无 404 | DevTools → Network，筛 `Status >= 400` |
| 下载按钮指向最新版 | 应跳到 `github.com/KversKv/soundlink-oss/releases/latest`，不含硬编码版本号 |
| 移动端无横向滚动 | DevTools 设备宽度 360px |
| 性能与可访问性 | Chrome Lighthouse 移动端；目标 Performance ≥ 90 / Accessibility ≥ 95 / Best Practices ≥ 95 / SEO ≥ 95（规划 §7） |
| 收录信息正确 | 直接访问 `/SoundLink/robots.txt` 与 `/SoundLink/sitemap.xml` |
| 分享卡片 | 需 `og.png` 就位后再验（当前缺失） |

## 7. 回滚

Pages 部署无内置版本回退，回滚等价于"重新发布旧内容"：

- 首选：`git revert` 出问题的提交并推 `main`，流水线自动重建。
- 应急：在 **Actions** 中找到上一次成功的 workflow run，点 **Re-run all jobs**。

## 8. 常见失败与处理

| 现象 | 原因 | 处理 |
|---|---|---|
| `deploy` job 报环境 / 权限错误 | Settings → Pages 的 Source 未切到 GitHub Actions | 见 §2 |
| checkout 失败或无权限 | 仓库默认 `GITHUB_TOKEN` 权限收紧时，`permissions` 未声明 `contents: read` | 在 `pages.yml` 的 `permissions` 补 `contents: read` |
| `npm ci` 失败提示 lock 不同步 | `package.json` 改了但未更新 `package-lock.json` | 本地 `npm install` 后提交 lock 文件 |
| 构建成功但页面白屏 | `base` 与实际访问路径不一致 | 对照 §5 逐项核对 |
| 字体解析告警 ×2 | `public/fonts/` 缺文件 | 见 §5 例外说明 |
| `dist/` 被误提交进仓库 | 根 `.gitignore` 未忽略 `website/dist/` | 在根 `.gitignore` 增加 `website/dist/` |

## 9. 关联文档

- 本地调试：[`01-local-dev.md`](./01-local-dev.md)
- 设计与验收规格：[`02-website-plan.md`](../../docs/NewFunctions/opensource-launch/02-website-plan.md)
- 发布总览：[`00-launch-overview.md`](../../docs/NewFunctions/opensource-launch/00-launch-overview.md)
