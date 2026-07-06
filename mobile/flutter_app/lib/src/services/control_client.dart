// 控制通道客户端（TCP，换行分帧 JSON）。
//
// 对齐 spec §3：每条消息一行 `\n` 结尾的 UTF-8 JSON。
// 桌面端 control_server.rs 监听 DEFAULT_CONTROL_PORT。
//
// 注：使用 dart:io Socket（仅 iOS/Android 可用，web 端不参与控制面）。

import 'dart:async';
import 'dart:convert';
import 'dart:io';

import '../protocol/control_message.dart';

typedef MessageHandler = void Function(Map<String, dynamic> msg);

class ControlClient {
  final String host;
  final int port;
  Socket? _socket;
  String _incoming = '';
  final _controllers = <StreamController<Map<String, dynamic>>>[];
  bool _connecting = false;

  ControlClient({required this.host, required this.port});

  bool get isConnected => _socket != null && !_connecting;

  /// 连接并启动接收循环。
  Future<void> connect() async {
    if (_socket != null) return;
    _connecting = true;
    _socket = await Socket.connect(host, port,
        timeout: const Duration(seconds: 5));
    _connecting = false;
    _socket!.listen(
      (List<int> data) {
        _incoming += utf8.decode(data);
        _drainLines();
      },
      onError: (Object e) => _notifyError('socket error: $e'),
      onDone: () => disconnect(),
    );
  }

  void _drainLines() {
    while (true) {
      final i = _incoming.indexOf('\n');
      if (i < 0) break;
      final line = _incoming.substring(0, i);
      _incoming = _incoming.substring(i + 1);
      if (line.isEmpty) continue;
      try {
        final msg = json.decode(line) as Map<String, dynamic>;
        for (final c in _controllers) {
          c.add(msg);
        }
      } catch (e) {
        // 忽略解析失败。
      }
    }
  }

  void _notifyError(String s) {
    for (final c in _controllers) {
      c.addError(s);
    }
  }

  /// 订阅入站消息流。
  Stream<Map<String, dynamic>> get messages {
    final c = StreamController<Map<String, dynamic>>();
    _controllers.add(c);
    c.onCancel = () => _controllers.remove(c);
    return c.stream;
  }

  /// 发送一条控制消息。
  void send(ControlMessage msg) {
    final frame = msg.toFrame();
    _socket?.add(utf8.encode(frame));
  }

  /// 发送原始 JSON 行。
  void sendRaw(Map<String, dynamic> obj) {
    _socket?.add(utf8.encode('${json.encode(obj)}\n'));
  }

  /// 等待满足 [test] 的下一条消息，超时抛 [TimeoutException]。
  Future<Map<String, dynamic>> waitFor(
    bool Function(Map<String, dynamic>) test, {
    Duration timeout = const Duration(seconds: 10),
  }) async {
    final completer = Completer<Map<String, dynamic>>();
    late StreamSubscription sub;
    sub = messages.listen((msg) {
      if (test(msg) && !completer.isCompleted) {
        completer.complete(msg);
        sub.cancel();
      }
    }, onError: (Object e) {
      if (!completer.isCompleted) completer.completeError(e);
    }, onDone: () {
      if (!completer.isCompleted) {
        completer.completeError(
          StateError('控制连接已断开（等待消息时 socket 关闭）'),
        );
      }
    });
    return completer.future.timeout(timeout, onTimeout: () {
      sub.cancel();
      throw TimeoutException('控制消息等待超时');
    });
  }

  void disconnect() {
    _socket?.destroy();
    _socket = null;
    for (final c in _controllers) {
      c.close();
    }
    _controllers.clear();
  }
}
