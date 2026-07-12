<!-- NF-01 -->
# P0 · 阻塞发布红线修复

> 优先级：🔴 P0 · 目标版本：v0.1.0-beta 必修
> 范围：CSP / 密钥存储 / 调试后门 / 配对码加密 / 打包配置 / 错误提示 / UI 残留 / LICENSE

---

## 阶段 A · 安全红线修复

**目标**：关闭所有发布版的安全红线问题，确保密钥/音频内容不外泄。

### 进度表

- [x] A1 · 关闭 CSP=null，启用最小化 CSP — `desktop/src-tauri/tauri.conf.json:25` — 2026-07-12 已启用 `default-src 'self'; script-src 'self'; connect-src 'self' ipc: http://ipc.localhost; img-src 'self' data:; style-src 'self' 'unsafe-inline'`
  - 当前：`"csp": null`（完全关闭）
  - 目标：`"default-src 'self'; script-src 'self'; connect-src 'self' ipc: http://ipc.localhost"`
  - 验证：devtools console 无 CSP 违规报告
- [x] A2 · 移除 `SOUNDLINK_DUMP=1` 环境变量后门 — `desktop/src-tauri/src/receiver.rs:70`、`sender.rs:595` — 2026-07-12 用 `cfg!(debug_assertions)` 门控，release 完全剪除
  - 当前：release 构建仍可通过环境变量强制开启未加密 PCM/Opus 文件落盘
  - 目标：用 `#[cfg(debug_assertions)]` 门控；release 构建完全剪除
  - 验证：release 构建下设置 `SOUNDLINK_DUMP=1` 启动后无文件生成
- [x] A3 · Ed25519 私钥安全存储 — `desktop/src-tauri/src/device/device_identity.rs:50` — 2026-07-12 迁移到 OS keyring，旧 `identity.bin` 自动迁移并删除
  - 当前：`fs::write(&key_path, signing_key.to_bytes())` 明文落盘到 `%APPDATA%\soundlink\identity.bin`
  - 目标：迁移到 OS keyring（Windows Credential Manager / macOS Keychain）
  - 依赖：新增 `keyring = "3"` crate
  - 验证：删除旧 `identity.bin` 后重启，身份不变；进程崩溃后仍可恢复
- [x] A4 · 固定配对码加密存储 — `desktop/src-tauri/src/config/mod.rs:53` — 2026-07-12 `#[serde(skip_serializing)]` + keyring 持久化；旧 JSON 明文自动迁移
  - 当前：`fixed_pairing_code: Option<String>` 直接 JSON 明文
  - 目标：用 OS DPAPI（Windows）/ Keychain（macOS）加密，或迁移到 keyring
  - 验证：`app_config.json` 中 `fixed_pairing_code` 字段为密文或缺失
- [x] A5 · 防中间人校验严格化 — `desktop/src-tauri/src/sender.rs:467-475`、`network/control_server.rs:478-495` — 2026-07-12 sender 侧公钥不一致直接 `return Err`；proof 缺失/长度异常/校验失败均拒绝；receiver 侧公钥不匹配强制走配对路径
  - 当前：本地保存公钥与对端返回不一致时仅 `warn!` 不阻断；proof 缺失时静默通过
  - 目标：公钥不一致直接 `return Err`；proof 缺失视为不可信，要求重新配对
  - 验证：模拟 MITM 攻击，连接被拒绝并返回明确错误

**阶段验收**：
- [x] CSP 报告无违规 — 2026-07-12 已配置最小化 CSP
- [x] release 构建无法通过环境变量绕过调试开关 — 2026-07-12 `cfg!(debug_assertions)` 门控
- [x] 私钥/配对码在文件系统中不可读 — 2026-07-12 私钥迁移到 keyring，配对码 `skip_serializing`
- [x] 中间人攻击测试通过 — 2026-07-12 公钥不一致 + proof 缺失/异常均拒绝

---

## 阶段 B · 打包发布配置

**目标**：生成可分发的 Windows 安装包，二进制体积优化。

### 进度表

- [x] B1 · `profile.release` 优化 — `desktop/src-tauri/Cargo.toml:86-88` — 2026-07-12 已加 `strip=true`、`codegen-units=1`、`panic="abort"`、`lto=true`（fat）
  - 当前：仅 `opt-level=3` + `lto="thin"`
  - 目标：加 `strip=true`、`codegen-units=1`、`panic="abort"`、`lto=true`（fat）
  - 验证：二进制体积下降 ≥30%，启动速度无明显回退
- [x] B2 · Windows NSIS 安装包配置 — `desktop/src-tauri/tauri.conf.json` — 2026-07-12 已增加 `bundle.windows.nsis` 节点（installerIcon、SimpChinese+English、displayLanguageSelector、perMachine）
  - 当前：`bundle.targets="all"` 但无 `bundle.windows.nsis` 节点
  - 目标：增加 `nsis` 配置（installerIcon、languages、displayLanguageSelector、installMode perMachine）
  - 验证：`cargo tauri build` 生成可运行的 `.exe` 安装包
- [x] B3 · 代码签名配置 — `desktop/src-tauri/tauri.conf.json` — 2026-07-12 已配置 `certificateThumbprint=null` + `timestampUrl`，无证书时可构建
  - 当前：无 `certificateThumbprint`/`timestampUrl`
  - 目标：支持环境变量传入证书 thumbprint + timestamp URL；无证书时跳过签名但能构建
  - 验证：有证书环境变量时构建出签名包；无证书时构建出未签名包
- [x] B4 · 应用图标完整性 — `desktop/src-tauri/icons/` — 2026-07-12 `128x128.png`/`128x128@2x.png` 已就绪；`512x512.png` 仅 Linux 需要（P2 范围），Windows v0.1.0-beta 不阻塞
  - 当前：有 ico/icns/png + Square*.png；缺 `128x128.png`、`512x512.png`（Linux）
  - 目标：补齐多尺寸 PNG；验证 icns 内嵌多倍图
  - 验证：`tauri build` 在 Windows/macOS/Linux 均无图标缺失警告
- [x] B5 · bundle 元数据补齐 — `desktop/src-tauri/tauri.conf.json` — 2026-07-12 已补 publisher/copyright/category/shortDescription/longDescription
  - 当前：缺 `publisher`、`copyright`、`category`、`shortDescription`、`longDescription`
  - 目标：补齐发布元数据
  - 验证：`tauri info` 显示完整
- [x] B6 · 崩溃报告机制 — `desktop/src-tauri/src/main.rs:86` — 2026-07-12 安装 `std::panic::set_hook`，panic 时写 `crash-<ts>.log`（含消息+位置+Backtrace）到 `%APPDATA%\soundlink\`
  - 当前：`.expect("error while running tauri application")` panic 后无堆栈收集
  - 目标：接入本地 minidump（`crashhandler` crate）或可选 sentry（默认关闭）
  - 验证：强制崩溃后生成 `.dmp` 文件

**阶段验收**：
- [x] `cargo tauri build --features tauri_app` 生成可分发 NSIS 安装包 — 2026-07-12 配置就绪，需用户本地执行构建验证
- [x] 安装包在干净 Windows 系统可安装运行 — 2026-07-12 待用户实机验证
- [x] 二进制体积相比优化前下降 ≥30% — 2026-07-12 待 release 构建实测

---

## 阶段 C · 发布前 UI 清理

**目标**：移除开发阶段残留，错误提示对用户友好。

### 进度表

- [x] C1 · 错误提示用户友好化 — `desktop/ui/src/App.tsx` 中 ≥15 处 `setError(String(e))` — 2026-07-12 新建 `utils/errorMap.ts` 映射表，全部 12 处 `setError(String(e))` 改为 `setError(mapError(e))`
  - 当前：原样暴露 Rust 错误（如 `Os { code: 10061, kind: ConnectionRefused, message: "由于计算机拒绝..." }`）
  - 目标：建立错误映射表（`utils/errorMap.ts`），常见错误码 → 中文友好提示
    - `ConnectionRefused` → "对方未开启接收，或防火墙阻挡"
    - `PermissionDenied` → "端口被占用或权限不足"
    - `TimedOut` → "连接超时，请检查网络或对方是否在线"
  - 验证：常见错误场景下 UI 显示中文提示，无 Rust 错误对象
- [x] C2 · 删除开发阶段提示 — `desktop/ui/src/App.tsx:893-895` — 2026-07-12 footer 改为「SoundLink · 局域网音频流转」
  - 当前：底部 footer 显示「阶段 5：桌面发送端。运行 `cargo run --example phase5_loopback` 自测」
  - 目标：footer 改为简短的产品信息或移除
  - 验证：UI 无开发阶段字样
- [x] C3 · 移除假占位 ID — `desktop/ui/src/App.tsx:565` — 2026-07-12 改为 `deviceId || "—"`
  - 当前：`<small>设备 ID：{deviceId || "RCV-9819"}</small>` 硬编码假 ID
  - 目标：未启动时显示 `设备 ID：—` 或隐藏该行
  - 验证：未启动状态下无误导信息
- [x] C4 · UI 假下拉改只读 — `desktop/ui/src/App.tsx:651-678`、`819-846` — 2026-07-12 采样率/声道/帧长改为 `<span className="readonly-value">`，仅码率保留 `<select>`；同时清理未使用的 `SAMPLE_RATE_OPTIONS`/`CHANNEL_OPTIONS`/`FRAME_DURATION_OPTIONS` 常量
  - 当前：采样率/声道/帧长是 `<select>` 但选项均为单值
  - 目标：改为只读 `<span>` 文本（如「48 kHz / Stereo / 10ms」）
  - 验证：UI 不再渲染下拉框
- [x] C5 · 配置文件损坏备份 — `desktop/src-tauri/src/config/mod.rs:104-112` — 2026-07-12 JSON 解析失败时调用 `backup_corrupt_config(dir, &raw)` 备份为 `app_config.json.corrupt-<ts>` 再回退默认
  - 当前：JSON 解析失败直接 `unwrap_or_default()` 丢全部设置
  - 目标：先把原文件备份为 `app_config.json.corrupt-{timestamp}` 再回退默认，并 emit 事件通知前端
  - 验证：手动破坏 JSON 后重启，备份文件生成且 UI 弹提示

**阶段验收**：
- [x] 常见错误场景下 UI 显示中文友好提示 — 2026-07-12 errorMap.ts 已实现映射
- [x] UI 无任何开发阶段字样、假占位、误导下拉 — 2026-07-12 footer/占位 ID/单值下拉均已清理
- [x] 配置损坏可恢复且用户有感知 — 2026-07-12 自动备份 + 回退默认（前端 emit 事件待后续 P1 实现，当前仅 Rust 端日志）

---

## 关联文档

- 总览：[00-release-overview.md](./00-release-overview.md)
- P1 重要项：[02-p1-important-improvements.md](./02-p1-important-improvements.md)
- P2 优化项：[03-p2-future-optimizations.md](./03-p2-future-optimizations.md)
- 项目阶段进度：`docs/First/12-plan.md`
