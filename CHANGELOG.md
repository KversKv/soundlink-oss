# 变更日志

本文件记录 SoundLink 的重要变更。格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [未发布]

## [0.1.0-beta.1] - 2026-08-04

首个公开内测版本（Pre-release）。仅实测通过 Android → Windows 与 Windows → Windows；macOS 采集未实装，iOS 待真机验收。产物未经代码签名，Windows 会触发 SmartScreen 提示。

### 新增

- 开源发布配套：`CONTRIBUTING.md`、`SECURITY.md`、`CODE_OF_CONDUCT.md`、`CHANGELOG.md`、Issue / PR 模板、GitHub Actions CI 工作流。
- 英文 README（`README.en.md`）。
- 开源发布待办规划文档 `docs/NewFunctions/opensource-launch/`：总览（OSL 阶段 J/K/L/M）与市场调研（竞品对比、差异化定位、推广渠道）。
- Release 工作流 `.github/workflows/release.yml`：`v*` tag 触发，构建 Windows 免安装 exe / NSIS 安装包与 Android APK，生成 SHA256 校验文件并创建 Draft Release。
- 版本管理体系：仓库根 `VERSION` 作为版本号单一来源，`scripts/sync_version.py` 同步至 `Cargo.toml` / `tauri.conf.json` / `desktop/ui/package.json` / `pubspec.yaml`；CI 增加 `version-check` 一致性门，Release 增加 `version-gate`（校验 tag 与 `VERSION` 相等）。
- 版本管理规划文档 `docs/NewFunctions/version-management/`：架构与实现计划（V1–V15）、版本号语义与递增判定规则（含大小版本区分、`0.x` 阶段规则、AI 协作代理的版本维护义务）。

### 变更

- **改版本号方式变更**：不再手改各端清单，统一编辑根 `VERSION` 后执行 `python scripts/sync_version.py`；手改清单会被 CI 一致性门拦截。
- 版本号统一为 `0.1.0-beta.1`：移动端 `pubspec.yaml` 此前为 Flutter 模板默认值 `1.0.0+1`，与桌面端 `0.1.0` 相差一个主版本，易被误认为已发布正式版，现已对齐。
- Android APK 的 `versionCode` 改由 CI `run_number` 注入，保证单调递增（此前固定为 1，多个 APK 无法覆盖安装）。
- `AGENTS.md` / `.trae/rules/project-rules.md` 增加版本维护约束：CHANGELOG 回填义务、禁止代理自行修改 `VERSION`、破坏性变更必须醒目标注。
- README 重写：补充问题陈述、终端用户使用步骤、已知限制、贡献方向；功能矩阵区分「实测可用 / 代码就绪未实测 / 未实装」。
- `mobile/README.md`、`mobile/ios/README.md`、`mobile/android/README.md` 去除过期的「占位骨架」描述，明确 `mobile/flutter_app` 为唯一构建入口。
- 发布就绪度总览结论更新为「具备 Windows Early Access 条件」，阶段 D 标记完成。

### 修复

- 修复 Rust 1.96 新增 clippy lint 导致 CI `Clippy (tauri_app)` 步骤失败：`Default::default()` 后字段赋值改为结构体更新语法、`&PathBuf` 参数改 `&Path`、`Option::map(identity)` 冗余调用移除；3 处协议/热路径函数与 Tauri command 的参数数量告警显式 `allow` 并注明原因。
- 仓库清理：移除冗余的 `.gitignore 2`，取消跟踪 `desktop/ui/tsconfig.tsbuildinfo` 构建缓存，根目录调试文档归档至 `docs/AI_Memory/Debug/`。

---

## 0.1.0-beta.1 首发包含的研发里程碑

以下能力在首个 Release（`v0.1.0-beta.1`）之前已在 `main` 上实现，随本版一并发布。

### 桌面端（Tauri 2 + Rust + React）

- 接收端全链路：mDNS 广播 → 配对码 → UDP 接收 → 解密 → 重排 / JitterBuffer → Opus 解码 → 时钟校正 → 设备输出。
- 发送端（Windows WASAPI Loopback）：系统音频采集 → Opus 编码 → 加密 → UDP 发送，支持 backoff 断线重连。
- 系统托盘、设置面板、开机自启动、单实例锁定、窗口状态记忆、全局快捷键、首次使用引导、关于页。
- 安全：CSP 收紧、OS keyring 存私钥与固定配对码、MITM 防护、配对码错误锁定。
- 打包：NSIS 安装包（简体中文 / 英文），macOS dmg 配置就绪。

### 移动端

- Flutter 主 App：设备发现、配对、状态展示、设置、广播引导。
- Android：MediaProjection + AudioPlaybackCapture 采集，libopus JNI 编码，前台 Service 与通知。
- iOS：ReplayKit Broadcast Extension 采集 / 编码 / 发送源码就绪（待真机验收）。

### 共享层

- `shared/`：协议消息、常量、错误码单源定义。
- 加密栈：ChaCha20-Poly1305 / X25519 / Ed25519 / HKDF-SHA256 / HMAC-SHA256。

### 已实测通过的端到端组合

- Android → Windows（2026-08-02）
- Windows → Windows（2026-08-02）

---

## 回填规则

1. 每次有用户可感知的变更，写入 `[未发布]` 对应小节（新增 / 变更 / 修复 / 移除 / 安全）。
2. 发版时把 `[未发布]` 改为 `[x.y.z] - YYYY-MM-DD`，并在其上新建空的 `[未发布]`。
3. 内部重构、纯文档微调可不记录；影响构建方式或使用方式的必须记录。
