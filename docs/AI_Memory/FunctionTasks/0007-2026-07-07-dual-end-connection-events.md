<!-- FT-0007 -->

# 双端连接事件管理与自动停流实录（2026-07-07）

> 场景：用户反馈电脑端停止接收或退出后手机仍持续传输，反向断开时桌面端也未自动清理，需要补齐双端连接状态监测、自动断开、自动停止与状态回收。

## 背景

- 控制通道采用 TCP JSON Lines，音频通道采用 UDP。
- 既有协议已包含 `heartbeat`、`stats`、`stream_stop`，但移动端未周期上报心跳，桌面 Receiver 未基于控制连接断开/心跳超时停流，桌面 Sender 只保留写半边，无法监听接收端关闭。
- Android 原生采集可由 Flutter 调用 `stopCapture` 停止；iOS ReplayKit Broadcast Extension 不能由主 App 直接强制停止，但可通过 App Group stop flag 让 Extension 自行结束广播。

## 实现清单

| 范围 | 文件 | 变更 |
|---|---|---|
| 桌面 Receiver | `desktop/src-tauri/src/network/control_server.rs` | 控制连接读取加入 `HEARTBEAT_TIMEOUT_SECS` 超时；stream_start 成功后标记活跃流；TCP EOF、心跳超时、stream_stop 统一触发 Receiver 停止接收。 |
| 桌面 Sender | `desktop/src-tauri/src/sender.rs` | 握手保留 TCP read half；控制循环同时读取接收端消息并发送 heartbeat/stats；接收端关闭、读取失败、写心跳/stats 失败时置 `DISCONNECTED` 并停止发送；接收端 `error` 置 `ERROR`。 |
| 移动控制客户端 | `mobile/flutter_app/lib/src/services/control_client.dart` | 新增远端断开广播流；区分手动断开与远端断开；连接关闭时关闭消息订阅并清空缓冲。 |
| 移动连接编排 | `mobile/flutter_app/lib/src/services/pairing_service.dart` | stream_start 后启动 heartbeat/stats 定时器；订阅控制断开并停止本地采集；手动停止发送 stream_stop 并断开控制通道。 |
| 移动状态 UI | `mobile/flutter_app/lib/src/models/connection_state.dart`、`mobile/flutter_app/lib/app.dart`、`mobile/flutter_app/lib/src/pages/home_page.dart`、`mobile/flutter_app/lib/src/pages/pairing_page.dart` | 新增 `reconnecting`/连接已断开状态，远端断开时显示错误提示并允许用户停止/清理。 |
| 平台停止语义 | `mobile/flutter_app/lib/src/services/platform_service.dart`、`mobile/flutter_app/android/app/src/main/kotlin/com/soundlink/soundlink/SoundLinkPlugin.kt`、`mobile/flutter_app/ios/Runner/SoundLinkPlugin.swift` | `stopCapture` 增加 `clearSession` 参数；远端断开保留会话配置以便 iOS Extension 读取 stop flag，手动停止清理配置。 |
| Android 采集 | `mobile/flutter_app/android/app/src/main/kotlin/com/soundlink/soundlink/capture/AudioCaptureService.kt` | `onDestroy` 补齐 AudioRecord、Encoder、UdpSender、MediaProjection 释放；停止路径清空引用；初始化失败时释放资源并停止服务。 |
| iOS Broadcast Extension | `mobile/ios/BroadcastExtension/PairingStateReader.swift`、`mobile/ios/BroadcastExtension/SampleHandler.swift` | App Group 增加 `soundlink.stop_requested`；Extension 以轻量 timer 轮询 stop flag，收到后调用 `finishBroadcastWithError` 自动结束广播。 |
| 进度同步 | `docs/First/12-plan.md` | 阶段 4 补记“双端连接事件管理与自动停流”完成记录与验证命令。 |

## 关键设计决策

- 不改协议 schema，复用已有 `heartbeat`、`stats`、`stream_stop`，避免同步修改协议文档。
- Receiver 只在 stream_start 成功后启用心跳超时停流，避免配对或等待用户输入配对码阶段被 6 秒超时误断。
- Flutter 控制断开事件只在非手动断开时触发，避免用户主动停止时重复进入异常状态。
- iOS 自动停止通过 App Group stop flag 让 Broadcast Extension 自行调用 ReplayKit 官方结束 API，不使用私有 API。
- 桌面 Sender 将接收端建议码率存入 `recommended_bitrate`，不覆盖实测发送码率 `bitrate`。

## 验证结果

- `dart format lib`：通过。
- `flutter analyze`：通过，No issues found。
- `cargo fmt`：通过。
- `cargo check --no-default-features`：通过。
- `cargo check --features tauri_app`：通过。

## 已知边界

- iOS 主 App 不能像 Android Service 一样直接杀掉 ReplayKit 广播；本次采用 Extension 轮询 App Group stop flag 并自行结束，生效间隔约 1 秒。
- 若手机进程被系统彻底杀死且无法执行控制通道关闭，桌面 Receiver 依靠 6 秒心跳超时停流。
- 若桌面进程被强杀，手机侧通过 TCP 关闭/写失败路径感知，最终触发本地采集停止。

## 关联文档

- 计划进度：`../../First/12-plan.md`
- 协议规格：`../../First/11-implementation-spec.md`
