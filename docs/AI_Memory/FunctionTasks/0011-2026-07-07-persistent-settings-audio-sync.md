<!-- FT-0011 -->

# 持久化设置、固定配对码与音频参数同步实录（2026-07-07）

> 场景：新增桌面固定配对码、移动端记忆上次设备/信任设备、桌面基础设置持久化、双端音频参数手动配置与自动推荐，并通过控制通道同步参数变化。

## 背景/需求

- 桌面端配对码支持随机/固定两种模式，固定码由用户手动输入 8 位数字。
- 移动端需要记忆上次连接设备和信任设备，并解释真机迭代/覆盖安装中的保存语义。
- 桌面端基础设置持久化到本机配置文件。
- 桌面端与移动端均支持常用音频参数手动选择和自动推荐。
- 参数变化通过已有 `control_action` 框架同步给对端。

## 实现清单

| 文件 | 变更 |
|---|---|
| `desktop/src-tauri/src/config/mod.rs` | 新增 `AppConfig`/`AudioParams` JSON 持久化，路径为 `dirs::config_dir()/soundlink/app_config.json` |
| `desktop/src-tauri/src/commands/mod.rs` | 桌面设置读写、固定配对码命令、音频参数命令、自动推荐、基础设置持久化 |
| `desktop/src-tauri/src/pairing/pairing_code.rs` | 配对码管理器支持用户固定 8 位数字，仍走 TTL/尝试次数/消费逻辑 |
| `desktop/src-tauri/src/network/control_server.rs` | `audio.params.update` 不再只 ACK：解析 payload、应用 Jitter、持久化参数，并在回执中标记是否需重启流 |
| `desktop/ui/src/App.tsx` | Receiver/Sender UI 增加固定配对码、音频参数、自动探测、持久化设置加载 |
| `desktop/ui/src/App.css` | 新增配对设置与音频参数表单样式 |
| `mobile/flutter_app/lib/app.dart` | 启动加载音频设置与上次 Receiver，连接成功后保存上次 Receiver，运行中发送参数更新 |
| `mobile/flutter_app/lib/src/services/trust_store.dart` | 基于 `shared_preferences` 保存 trusted receivers、last receiver、audio settings |
| `mobile/flutter_app/lib/src/services/pairing_service.dart` | `hello`、`stream_start`、原生 `SessionConfig` 使用音频设置，并发送 `audio.params.update` |
| `mobile/flutter_app/lib/src/pages/settings_page.dart` | 移动端音频参数常用选项与自动推荐入口 |
| `mobile/flutter_app/lib/src/pages/discovery_page.dart` | 展示上次连接设备并支持快速连接 |
| `docs/First/04-protocol.md` | 补充音频参数控制动作与运行时/下次流生效语义 |
| `docs/First/11-implementation-spec.md` | 补充 `audio.params.*` schema、可选值、`restart_required` 回执语义 |
| `docs/First/12-plan.md` | 回填固定配对码、持久化设置、移动端记忆设备、音频参数同步与自动推荐进度 |

## 关键设计决策

- 固定配对码保存在桌面本机配置中，UI 显示风险提示；代码不写日志，避免泄露。
- 固定配对码只影响 `issue()` 生成值，不绕过 `verify()` 的有效期、尝试次数与成功消费逻辑。
- 移动端现阶段继续使用 `shared_preferences`：热重启、热重载、覆盖安装/正常更新通常保留；卸载重装、清除 App 数据、改变 Android `applicationId` 或 iOS bundle id 会丢失。
- 音频参数第一版支持持久化与协议同步；`jitter_mode` 可运行时立即应用，`bitrate` 由发送端后续编码应用，采样率/声道/帧长完整动态生效需下次流或后续更深层音频管线改造。
- 自动推荐使用现有丢包率、抖动和 recommended bitrate 统计，不引入 WebRTC 或额外重型探测栈。

## 验证结果

- `cargo check --no-default-features` 通过。
- `cargo check --features tauri_app` 通过。
- `desktop/ui npm run build` 通过。
- `flutter analyze` 通过。
- IDE 诊断无问题。

## 已知边界

- 桌面 Rust Opus codec、packet header、发送 ticker 与接收播放链路仍主要围绕 48kHz/Stereo/10ms 基线，非默认采样率/声道/帧长按下次流/后续改造处理。
- Android 原生 capture loop 与 iOS `AudioProcessor` 当前仍有 48kHz/Stereo/10ms 固定路径，编码器与 packet header 已能读取配置，但全链路非默认参数仍需进一步原生改造。
- 桌面端本次主要完成 Receiver 侧接收 `audio.params.update`；桌面 Sender 运行中主动向对端推送参数变化仍可后续扩展。

## 关联文档

- [FT-0010](./0010-2026-07-07-generic-control-action-protocol.md)
