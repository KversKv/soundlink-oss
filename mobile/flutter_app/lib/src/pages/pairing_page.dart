// 配对页：输入配对码，连接并启动采集；显示连接状态与错误。

import 'package:flutter/material.dart';

import '../../app.dart';
import '../../main.dart' show DEBUG;
import '../models/connection_state.dart';

class PairingPage extends StatefulWidget {
  final AppState app;
  const PairingPage({super.key, required this.app});

  @override
  State<PairingPage> createState() => _PairingPageState();
}

class _PairingPageState extends State<PairingPage> {
  final _codeCtrl = TextEditingController();
  bool _busy = false;

  @override
  void initState() {
    super.initState();
    // DEBUG 模式下默认填充固定配对码（与桌面端 DEBUG 模式生成的码一致）。
    if (DEBUG) {
      _codeCtrl.text = '12345678';
    }
  }

  @override
  void dispose() {
    _codeCtrl.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final app = widget.app;
    return Scaffold(
      appBar: AppBar(title: const Text('配对')),
      body: ListenableBuilder(
        listenable: app,
        builder: (context, _) {
          final device = app.selectedDevice;
          return ListView(
            padding: const EdgeInsets.all(20),
            children: [
              Text('目标设备', style: Theme.of(context).textTheme.titleSmall),
              const SizedBox(height: 4),
              Text(
                device != null
                    ? '${device.deviceName}\n${device.host}'
                    : '未选择（请在“设备”页选择）',
              ),
              const SizedBox(height: 24),
              if (app.lastError != null)
                Container(
                  padding: const EdgeInsets.all(10),
                  color: Colors.red.shade50,
                  child: Text(
                    app.lastError!,
                    style: const TextStyle(fontSize: 13),
                  ),
                ),
              const SizedBox(height: 16),
              TextField(
                controller: _codeCtrl,
                decoration: const InputDecoration(
                  labelText: '配对码（8 位数字）',
                  hintText: '查看电脑端显示的配对码',
                  border: OutlineInputBorder(),
                ),
                keyboardType: TextInputType.number,
                maxLength: 8,
              ),
              const SizedBox(height: 16),
              FilledButton.icon(
                onPressed: _busy || device == null
                    ? null
                    : () => _connect(app, trusted: false),
                icon: const Icon(Icons.link),
                label: const Text('配对并开始广播'),
              ),
              const SizedBox(height: 8),
              OutlinedButton.icon(
                onPressed: _busy || device == null
                    ? null
                    : () => _connect(app, trusted: true),
                icon: const Icon(Icons.history),
                label: const Text('已信任设备直接连接'),
              ),
              const SizedBox(height: 24),
              if (app.conn == LinkState.streaming)
                FilledButton.tonalIcon(
                  onPressed: _busy ? null : () => app.stop(),
                  icon: const Icon(Icons.stop),
                  label: const Text('停止广播'),
                  style: FilledButton.styleFrom(
                    backgroundColor: Colors.red.shade100,
                  ),
                ),
            ],
          );
        },
      ),
    );
  }

  Future<void> _connect(AppState app, {required bool trusted}) async {
    setState(() => _busy = true);
    try {
      await app.connectAndStart(pairingCode: trusted ? null : _codeCtrl.text);
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }
}
