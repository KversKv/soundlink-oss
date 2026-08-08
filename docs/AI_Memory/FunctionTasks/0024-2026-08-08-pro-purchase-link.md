<!-- FT-0024 -->
# Pro 购买入口接入真实渠道（2026-08-08）

> 场景：Pro 授权底座与签发工具链已就绪（[FT-0021](./0021-2026-08-06-open-core-pro-implementation.md)、[FT-0023](./0023-2026-08-08-pro-license-pubkey-mismatch-fix.md)），但购买入口仍是占位地址 `https://soundlink.example.com/pro`。用户提供淘宝小店真实链接，要求接入官网与桌面端授权区。

## 实现清单

购买链接统一为 `https://e.tb.cn/h.85qPaoTYnCLX5Js?tk=YNdLgzkcHTH`（淘宝小店，渠道见 `docs/NewFunctions/monetization/00-monetization-overview.md`）。

| 位置 | 文件 | 改动 |
|---|---|---|
| 桌面端授权区 | `desktop/ui/src/components/LicensePanel.tsx` | 新增模块级常量 `PRO_PURCHASE_URL`，「购买 Pro（￥9.99 买断）」按钮由占位地址改指真实链接 |
| 官网文案（中） | `website/src/content/zh.ts` | 新增 `PRO_STORE` 常量；`links.pro`、`download.proCta`、页脚「项目」列「购买 Pro」链接 |
| 官网文案（英） | `website/src/content/en.ts` | 键名镜像 zh（`Content` 类型强制一致）；`proCta` 为 `Buy Pro · ¥9.99 one-time` |
| 官网下载区 | `website/src/sections/Download.tsx` | CTA 行新增第三个 ghost 按钮「购买 Pro」，指向 `c.links.pro` |
| 变更日志 | `CHANGELOG.md` | `[未发布]` 新增条目 |

## 关键设计决策

- **桌面端无需改 Tauri 能力配置**：`capabilities/default.json` 的 `opener:default` 已含 `allow-open-url`（允许 http/https），`plugin:opener|open_url` 直接可开淘宝链接，与设置页既有外链同一通路。
- **入口选位**：官网入口放在下载区（CTA 语境最贴切）与页脚项目列；导航栏不加，保持开源站「免费核心」主叙事，Pro 仅作补充入口。
- **链接未抽进 `shared/`**：website 与 desktop/ui 是两个独立 Vite 工程、无既有共享 TS 常量通道；各自文件内单常量定义，注释注明权威来源文档。

## 验证结果

- `website`：`tsc -b` 通过（exit 0），zh/en 键一致性由 `Content` 类型编译期强制。
- `desktop/ui`：`tsc -b` 通过（exit 0）。

## 追加（同日）：顶部「文档」导航改指 README

- 用户要求官网顶部「文档」跳转 oss 的 README 而非 `docs/user`：`zh.ts` `links.docs` 改为 `${REPO}/blob/main/README.md`，`en.ts` 改为 `${REPO}/blob/main/README.en.md`（英文用户落英文 README）。页脚「用户文档」链接保持 `docs/user` 不变。
- 用户要求官网顶部「下载 Beta」左侧新增「购买 Pro」按钮：zh/en `nav` 新增 `proCta`（购买 Pro / Buy Pro），`Nav.tsx` 桌面与移动端菜单各加一个 ghost 风格按钮指向 `c.links.pro`。
- 验证：`website` `tsc -b` 通过；CHANGELOG `[未发布]` 变更节已回填。

## 追加（同日）：官网主页新增「快速分辨率切换」分区

- 用户要求在主页描述 QR-1（快速分辨率切换，Pro），素材取 `oss/temp/png/` 两张真实截图：设置面板（`desktop-quick-resolution.png`）与托盘菜单（`tray-quick-resolution.png`），已复制进 `website/public/assets/`。
- 新增 `website/src/sections/QuickResolution.tsx`：左文案（Pro 徽标 + 标题 + 场景化描述 + 3 条要点 + 边界说明）右双截图交叠布局（复用 Scenario 的叠图手法），插入 `App.tsx` 中 Differentiators 之后；zh/en 新增 `quickResolution` 文案块。
- 文案锚定真实能力：预设档位/拖拽排序/系统导入、托盘一键切换 + 15s 确认回滚、退出恢复原始分辨率；note 标注「仅 Windows 官方版；NVIDIA 自定义分辨率与 EDID 预置为实验能力默认关闭」。
- 验证：`website` `npm run build`（tsc + vite）通过；CHANGELOG `[未发布]` 变更节已回填。

## 追加（同日）：分区顺序调整 + 终端用户文档

- 快速分辨率切换分区由「三条差异化」之后移至**「平台支持」上方**（Hero 之后）。
- 新增 `docs/user/user-readme.md`：面向终端用户的入口文档（安装含 SmartScreen/防火墙、快速上手、已知限制、免费 vs Pro 表 + QR-1 行、淘宝下单与离线激活流程、简版 FAQ），已挂入 `docs/user/00-index.md` 索引首位。
- 官网顶部「文档」导航由 README 改指 `docs/user/user-readme.md`（zh/en 同指中文文档——桌面 UI 当前仅中文，与 README 的「文档导航」中开发者文档保持区分）。
- 验证：`website` `tsc -b` 通过；CHANGELOG `[未发布]` 新增/变更节已回填。

## 用户需自行完成部分

- 桌面端实机点击「购买 Pro」按钮验证系统浏览器跳转（`open_url` 行为与设置页既有外链一致，预期无差异）。
- 若后续增加爱发电渠道，需在各入口并列第二链接。

## 版本建议

**不升版本**。理由：仅落地既有规划的购买入口（内容与链接接线），无协议/能力变更；已按义务 A 写入 CHANGELOG `[未发布]`，随下个含 Pro 的版本一并发布即可。

## 关键文件索引

- `desktop/ui/src/components/LicensePanel.tsx`（`PRO_PURCHASE_URL`）
- `website/src/content/zh.ts` / `en.ts`（`PRO_STORE`）
- `website/src/sections/Download.tsx`
- `desktop/src-tauri/capabilities/default.json`（opener 权限，未改动）
