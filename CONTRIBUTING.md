# 贡献指南 · Contributing to SoundLink

感谢关注 SoundLink。本项目当前处于早期阶段（Windows + Android 已实测可用，其他平台待补全），**最需要的贡献是真机验证与平台补全**。

阅读本文前建议先看 [`README.md`](README.md) 与 [`AGENTS.md`](AGENTS.md)。

---

## 1. 最需要帮助的方向

| 方向 | 说明 | 相关文档 |
|---|---|---|
| macOS 接收端验证 | 代码就绪（cpal / CoreAudio）但未实测，需 macOS 机器跑通 | `docs/user/02-dev-env-desktop.md` |
| macOS 发送端实装 | `desktop/src-tauri/src/audio/capture/macos_screencapturekit.rs` 目前是占位 | `docs/NewFunctions/release-readiness/03-p2-future-optimizations.md` G1 |
| Linux 接收端实装 | `linux_pipewire.rs` 仅注释 | 同上 G2 |
| iOS 真机验收 | Broadcast Extension 源码就绪，缺真机验证 | `docs/user/03-dev-env-ios.md` |
| Android 机型兼容 | 不同厂商 ROM 的 MediaProjection / AudioPlaybackCapture 行为差异 | `docs/First/08-platform-notes.md` |
| i18n | UI 目前仅中文，需要英文及其他语言 | 03-p2 文档 I3 |
| 文档与排查手册 | 补充实际遇到的问题与解法 | `docs/user/08-troubleshooting.md` |

不确定从哪开始？开一个 Issue 说明你的平台与设备，我们一起定范围。

---

## 2. 开发环境

### 桌面端（Windows / macOS / Linux）

```bash
# 前置：Rust stable 1.80+、Node.js 20、CMake + C 编译器、Tauri 2 前置依赖
cd desktop/ui
npm install
cd ../src-tauri
cargo tauri dev --features tauri_app
```

### 移动端

```bash
cd mobile/flutter_app
flutter pub get
flutter run -t lib/main.dart          # Android
```

iOS 需 macOS + Xcode，先执行 `ios/scripts/build_opus_xcframework.sh`。

详细环境搭建见 [`docs/user/01-dev-env-common.md`](docs/user/01-dev-env-common.md) 起的系列文档。

---

## 3. 提 PR 前必须通过的检查

### 桌面端 Rust

```bash
cd desktop/src-tauri
cargo fmt --all -- --check
cargo clippy --features tauri_app --all-targets -- -D warnings
cargo test --features opus
```

### 桌面端前端

```bash
cd desktop/ui
npm run build      # 含 tsc 类型检查
```

### 移动端

```bash
cd mobile/flutter_app
dart format --output=none --set-exit-if-changed lib test
flutter analyze
flutter test
```

CI（`.github/workflows/ci.yml`）会跑同样的检查。**CI 未通过的 PR 不会被合并。**

---

## 4. 代码约定

- **协议与常量单源**：任何协议字段、端口、魔法值必须定义在 [`shared/`](shared/README.md)，禁止在各端硬编码。改协议同时更新 [`docs/First/04-protocol.md`](docs/First/04-protocol.md) 与 [`docs/First/11-implementation-spec.md`](docs/First/11-implementation-spec.md)。
- **音频基线不随意变更**：48 kHz / Stereo / Opus 10 ms / 128 kbps / 默认 Jitter 80 ms。运行时可变参数仅 Opus 码率、Jitter、桌面音量。
- **日志**：Rust 用 `tracing`，禁 `println!`；密钥、配对码、私钥**禁止落日志**。
- **移动端采集侧保持轻量**：iOS Broadcast Extension 与 Android Service 内不引入重依赖、不做大缓冲。
- **合规红线**：不使用私有 API，不要求 root/越狱，不试图绕过 DRM。
- **最小改动**：一个 PR 只做一件事，不夹带无关重构。

---

## 5. 提交与分支

- 分支命名：`feat/xxx`、`fix/xxx`、`docs/xxx`、`chore/xxx`。
- 提交信息用祈使句、简明说明「为什么」，例如：
  ```
  fix(receiver): 避免 UDP recv 错误直接退出接收循环
  ```
- PR 描述请填 [`.github/pull_request_template.md`](.github/pull_request_template.md) 中的各项，特别是**实测平台与设备**。

---

## 6. 报告问题

- Bug：用 [Bug 报告模板](.github/ISSUE_TEMPLATE/bug_report.yml)，务必附平台、版本、日志片段与网络环境。
- 功能建议：用 [功能建议模板](.github/ISSUE_TEMPLATE/feature_request.yml)。
- **安全漏洞：不要开公开 Issue**，按 [`SECURITY.md`](SECURITY.md) 私下反馈。

日志位置与抓取方式见 [`docs/user/06-debug.md`](docs/user/06-debug.md)。

---

## 7. 许可

提交贡献即表示同意你的代码以 [MIT 许可证](LICENSE) 发布。
