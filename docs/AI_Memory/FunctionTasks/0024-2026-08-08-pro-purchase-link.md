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
- 验证：`website` `tsc -b` 通过；CHANGELOG `[未发布]` 变更节已回填。

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
