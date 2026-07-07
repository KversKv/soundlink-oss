<!-- FT-0009 -->

# 手动停止双端联动修复实录（2026-07-07）

> 场景：用户实测发现单端手动停止接收或广播时，另一端不会自动停止。上一轮实现只覆盖控制连接 EOF、心跳超时、读写失败等异常路径，未覆盖主动停止路径。

## 背景

SoundLink 控制面使用 TCP JSON Lines，音频面使用 UDP。`stream_stop` 是既有协议消息，本次未新增协议类型，重点是让手动停止路径也走控制面通知，而不是只停止本地任务。

## 根因分析

| 问题 | 根因 |
|---|---|
| 电脑端手动停止 Receiver 后手机仍在广播 | `ControlServer::stop()` 只停止 listener 和本地 ReceiverEngine，没有通知已有控制连接，也没有主动关闭已连接 TCP |
| 桌面 Sender 手动停止后 Receiver 不一定停止 | `SenderEngine::stop()` 直接 abort 控制任务，导致 `control_loop` 末尾发送 `stream_stop` 的逻辑没有机会执行 |
| 手机手动停止后桌面端偶发收不到 `stream_stop` | Flutter `control.send()` 是 fire-and-forget，发送后立即 destroy socket，可能未 flush |
| 手机端无法响应 Receiver 主动 `stream_stop` | Flutter 端此前只监听 socket 断开，未监听入站 `stream_stop` / `error` |

## 实现清单

| 文件 | 变更 |
|---|---|
| `desktop/src-tauri/src/network/control_server.rs` | 为 Receiver 控制服务器增加 `Notify` 停止广播；手动停止时向活动连接发送 `stream_stop` 并 shutdown TCP |
| `desktop/src-tauri/src/sender.rs` | Sender 停止改为 async graceful stop；通过 `Notify` 唤醒 control loop，自然发送 `stream_stop` 后关闭 writer；支持接收端入站 `stream_stop` |
| `desktop/src-tauri/src/commands/mod.rs` | `stop_sender` 改为 async command，等待 Sender graceful stop 完成 |
| `desktop/src-tauri/examples/phase5_loopback.rs` | 适配 async `sender.stop().await` |
| `mobile/flutter_app/lib/src/services/control_client.dart` | 新增 `sendAndFlush()`，用于关键控制帧发送后等待 socket flush |
| `mobile/flutter_app/lib/src/services/pairing_service.dart` | 监听入站 `stream_stop` / `error` 自动停止原生采集；手动 stop 时 await flush `stream_stop` |
| `mobile/flutter_app/lib/app.dart` | 调整自动停止提示文案，覆盖对端主动停止场景 |
| `docs/First/12-plan.md` | 回填双端事件管理任务，补充手动停止互相通知说明 |

## 关键设计决策

- 复用既有 `stream_stop` 消息，不新增协议类型，保持与 `04-protocol` / `11-implementation-spec` 一致。
- Receiver 手动停止时优先发送 `stream_stop`，再 shutdown TCP；即使对端未处理消息，也会通过 EOF 触发停止兜底。
- Sender 手动停止不再 abort 控制任务，而是通知控制循环退出，让它执行尾部 `stream_stop` 和 writer shutdown。
- Flutter 手动 stop 对 `stream_stop` 使用 flush，避免发送后立即销毁 socket 导致控制帧丢失。
- Flutter 同时监听入站 `stream_stop` 和 socket 断开，覆盖“对端正常停止”和“对端异常退出”两类事件。

## 验证结果

| 命令 | 结果 |
|---|---|
| `dart format lib` | 通过 |
| `flutter analyze` | 通过，No issues found |
| `cargo fmt` | 通过 |
| `cargo check --no-default-features` | 通过 |
| `cargo check --features tauri_app` | 通过 |

## 预期行为

- 手机端手动停止广播：先 flush `stream_stop`，桌面 Receiver 收到后停止接收并清理状态。
- 电脑端手动停止接收：Receiver 控制服务器向手机/桌面 Sender 发送 `stream_stop`，随后关闭控制连接；手机端收到后停止原生采集，桌面 Sender 收到后进入断开状态。
- 桌面 Sender 手动停止：控制循环发送 `stream_stop` 后关闭 TCP，桌面 Receiver 立即停止接收。
- 如果控制消息未送达，TCP 关闭与心跳超时仍作为兜底停止路径。

## 已知边界

- iOS 主 App 不能直接强杀 ReplayKit Broadcast Extension；自动停止依赖 App Group stop flag 与 Extension 轮询，最多约 1 秒响应。
- 如果网络完全断开导致 TCP 关闭事件无法即时传递，仍依赖心跳超时兜底。

## 关联文档

- [FT-0007](./0007-2026-07-07-dual-end-connection-events.md)
