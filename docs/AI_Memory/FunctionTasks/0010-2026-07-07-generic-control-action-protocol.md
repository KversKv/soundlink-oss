<!-- FT-0010 -->

# 通用控制动作协议补齐（2026-07-07）

> 场景：用户询问当前双端互相通知流程是否为通用格式，并提出后续需要支持通信、快捷指令设置、上一曲、播放/暂停、下一曲等控制流程。

## 方案

保留 `stream_start` / `stream_stop` 作为音频流生命周期消息，新增通用 `control_action` / `control_action_ack` 作为低频动作 envelope。这样不会污染音频状态机，也方便后续把媒体键、快捷指令、设备控制统一到同一解析入口。

## 协议格式

```json
{
  "type": "control_action",
  "msg_id": "c-action-1",
  "ts": 1730000005000,
  "action": "media.play_pause",
  "target": "receiver",
  "correlation_id": "optional-id",
  "payload": {}
}
```

```json
{
  "type": "control_action_ack",
  "msg_id": "s-action-1",
  "ts": 1730000005010,
  "reply_to": "c-action-1",
  "action": "media.play_pause",
  "result": "accepted"
}
```

## 预留动作

| action | 说明 |
|---|---|
| `media.play_pause` | 播放/暂停 |
| `media.previous` | 上一曲 |
| `media.next` | 下一曲 |
| `shortcut.set` | 设置快捷指令 |
| `shortcut.trigger` | 触发快捷指令 |

## 实现清单

| 文件 | 变更 |
|---|---|
| `desktop/src-tauri/src/network/control_server.rs` | 增加 `CONTROL_ACTION` / `CONTROL_ACTION_ACK` 消息类型和 Receiver 侧统一解析回执 |
| `desktop/src-tauri/src/sender.rs` | Sender 侧识别 `control_action` / `control_action_ack`，为后续处理器接入预留入口 |
| `mobile/flutter_app/lib/src/protocol/control_message.dart` | 增加 `ControlActionMsg`、`ControlActionAckMsg` 与 `ControlActions` 常量 |
| `shared/protocol/README.md` | 同步通用动作 envelope 与预留动作名 |
| `docs/First/04-protocol.md` | 同步控制消息总览与通用动作说明 |
| `docs/First/11-implementation-spec.md` | 同步字段级 schema、回执 result、预留动作 payload |
| `docs/First/12-plan.md` | 回填事件管理任务说明，补充通用控制动作 envelope |

## 验证结果

| 命令 | 结果 |
|---|---|
| `cargo fmt` | 通过 |
| `dart format lib` | 通过 |
| `cargo check --no-default-features` | 通过 |
| `cargo check --features tauri_app` | 通过 |
| `flutter analyze` | 通过 |

## 后续接入点

- 桌面 Receiver：在 `handle_control_action` 中把 action 分发到媒体键或快捷指令执行器。
- 桌面 Sender：在 `handle_control_message` 的 `CONTROL_ACTION` 分支接入对端动作处理。
- 移动端：使用 `ControlActionMsg` 发送媒体键或快捷指令请求，并根据 `control_action_ack` 更新 UI 状态。
- 快捷指令配置：建议 `shortcut.set` 的 payload 后续约束为 `{ id, binding, action, payload }`，避免 UI 设置和执行协议分裂。

## 关联文档

- [FT-0009](./0009-2026-07-07-manual-stop-propagation.md)
