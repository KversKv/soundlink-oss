## 变更内容

<!-- 简述这个 PR 做了什么，以及为什么需要 -->

关联 Issue：Closes #

## 变更类型

- [ ] Bug 修复
- [ ] 新功能
- [ ] 文档
- [ ] 构建 / CI
- [ ] 重构（无行为变化）

## 影响范围

- [ ] 桌面接收端
- [ ] 桌面发送端
- [ ] Android
- [ ] iOS
- [ ] 协议 / 共享层（`shared/`）
- [ ] 文档

## 实测情况

> 请填写你实际验证过的平台与设备，未实测的组合请明确说明。

| 组合 | 是否实测 | 设备 / 系统版本 |
|---|---|---|
| 发送端 → 接收端 |  |  |

## 检查清单

- [ ] 已阅读 [CONTRIBUTING.md](../CONTRIBUTING.md)
- [ ] `cargo clippy --features tauri_app --all-targets -- -D warnings` 无告警（若改 Rust）
- [ ] `cargo test --features opus` 通过（若改 Rust）
- [ ] 新增/修改的 Rust 代码已 `cargo fmt`
- [ ] `npm run build` 通过（若改前端）
- [ ] `flutter analyze` + `flutter test` 通过（若改移动端）
- [ ] 未在日志中输出配对码 / 密钥 / 私钥
- [ ] 若改动协议或常量，已同步 `shared/` 与 `docs/First/04-protocol.md`、`11-implementation-spec.md`
- [ ] 若改动音频基线或运行时参数，已在 PR 描述中说明理由
- [ ] 用户可感知的变更已写入 [CHANGELOG.md](../CHANGELOG.md) 的 `[未发布]` 小节
- [ ] 未引入私有 API / root / 越狱 / DRM 绕过

## 补充说明

<!-- 截图、日志、已知限制等 -->
