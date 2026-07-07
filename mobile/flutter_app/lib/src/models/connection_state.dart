// 发送端连接状态机。对齐 spec §6.1。
//
// 命名为 LinkState 以避免与 Flutter 内置 ConnectionState 冲突。

enum LinkState {
  disconnected,
  connecting,
  connected,
  pairing,
  paired,
  streaming,
  reconnecting,
  error,
}

extension LinkStateX on LinkState {
  String get label => switch (this) {
    LinkState.disconnected => '未连接',
    LinkState.connecting => '连接中',
    LinkState.connected => '已连接',
    LinkState.pairing => '配对中',
    LinkState.paired => '已配对',
    LinkState.streaming => '正在广播',
    LinkState.reconnecting => '连接已断开',
    LinkState.error => '错误',
  };
}
