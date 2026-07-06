// 设备发现页：扫描局域网桌面 Receiver，列表选择；支持手动输入 IP。

import 'package:flutter/material.dart';

import '../../app.dart';

class DiscoveryPage extends StatelessWidget {
  final AppState app;
  const DiscoveryPage({super.key, required this.app});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('设备发现')),
      body: ListenableBuilder(
        listenable: app,
        builder: (context, _) {
          if (app.scanning && app.devices.isEmpty) {
            return const Center(child: CircularProgressIndicator());
          }
          return Column(
            children: [
              if (app.lastError != null)
                _banner(app.lastError!, Colors.red.shade50),
              if (app.devices.isEmpty)
                const Padding(
                  padding: EdgeInsets.all(24),
                  child: Text('未发现设备，确认电脑端已开启接收模式，或手动输入 IP。',
                      textAlign: TextAlign.center),
                ),
              Expanded(
                child: ListView.builder(
                  itemCount: app.devices.length,
                  itemBuilder: (_, i) {
                    final d = app.devices[i];
                    final selected = d == app.selectedDevice;
                    return ListTile(
                      leading: const Icon(Icons.speaker),
                      title: Text(d.deviceName),
                      subtitle: Text('${d.host}  ·  音频端口 ${d.audioPort}'),
                      trailing: selected
                          ? const Icon(Icons.check_circle, color: Colors.green)
                          : null,
                      onTap: () => app.selectDevice(d),
                    );
                  },
                ),
              ),
              Padding(
                padding: const EdgeInsets.all(12),
                child: Row(
                  children: [
                    Expanded(
                      child: OutlinedButton.icon(
                        onPressed: app.scanning ? null : () => app.scan(),
                        icon: const Icon(Icons.refresh),
                        label: Text(app.scanning ? '扫描中…' : '重新扫描'),
                      ),
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      child: OutlinedButton.icon(
                        onPressed: () => _showManualDialog(context),
                        icon: const Icon(Icons.edit),
                        label: const Text('手动 IP'),
                      ),
                    ),
                  ],
                ),
              ),
            ],
          );
        },
      ),
    );
  }

  void _showManualDialog(BuildContext context) {
    final ipCtrl = TextEditingController();
    showDialog(
      context: context,
      builder: (_) => AlertDialog(
        title: const Text('手动输入 IP'),
        content: TextField(
          controller: ipCtrl,
          decoration: const InputDecoration(
              hintText: '192.168.1.10', labelText: '桌面端 IP'),
          keyboardType: TextInputType.number,
        ),
        actions: [
          TextButton(
              onPressed: () => Navigator.pop(context), child: const Text('取消')),
          FilledButton(
            onPressed: () {
              final ip = ipCtrl.text.trim();
              if (ip.isNotEmpty) {
                app.addManualDevice(ip);
                Navigator.pop(context);
              }
            },
            child: const Text('添加'),
          ),
        ],
      ),
    );
  }

  Widget _banner(String text, Color bg) => Container(
        width: double.infinity,
        color: bg,
        padding: const EdgeInsets.all(8),
        child: Text(text, style: const TextStyle(fontSize: 13)),
      );
}
