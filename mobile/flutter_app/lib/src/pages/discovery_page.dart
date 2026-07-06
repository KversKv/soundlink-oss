// 设备发现页：扫描局域网桌面 Receiver，列表选择；支持手动输入 IP。
// 已信任设备可直接快速重连（跳过配对码）。

import 'package:flutter/material.dart';

import '../../app.dart';
import '../../main.dart' show DEBUG, DUMP_ENABLE;
import '../models/device.dart';
import '../services/trust_store.dart';

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
          return ListView(
            children: [
              if (app.lastError != null)
                _banner(app.lastError!, Colors.red.shade50),
              if (app.trustedReceivers.isNotEmpty) ...[
                const _SectionHeader('已信任设备（点击直接连接）'),
                ...app.trustedReceivers.map((t) => _trustedTile(context, t)),
                const Divider(),
              ],
              const _SectionHeader('发现的设备'),
              if (app.scanning && app.devices.isEmpty)
                const Padding(
                  padding: EdgeInsets.all(24),
                  child: Center(child: CircularProgressIndicator()),
                )
              else if (app.devices.isEmpty)
                const Padding(
                  padding: EdgeInsets.all(24),
                  child: Text(
                    '未发现设备，确认电脑端已开启接收模式，或手动输入 IP。',
                    textAlign: TextAlign.center,
                  ),
                )
              else
                ...app.devices.map((d) => _deviceTile(d)),
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
              // 调试：采集 PCM 转储开关
              _DumpPcmTile(app: app),
            ],
          );
        },
      ),
    );
  }

  Widget _trustedTile(BuildContext context, TrustedReceiver t) {
    return ListTile(
      leading: const Icon(Icons.verified, color: Colors.green),
      title: Text(t.deviceName),
      subtitle: Text('${t.host}  ·  已信任'),
      trailing: IconButton(
        icon: const Icon(Icons.delete_outline, size: 20),
        onPressed: () => app.removeTrusted(t.deviceId),
      ),
      onTap: () {
        final device = DiscoveredDevice(
          deviceId: t.deviceId,
          deviceName: t.deviceName,
          role: 'receiver',
          protocolVersion: 1,
          pairingRequired: true,
          audioCodec: 'opus',
          sampleRate: 48000,
          controlPort: t.controlPort,
          audioPort: t.audioPort,
          host: t.host,
        );
        app.selectDevice(device);
        // 已信任设备直接连接（无配对码）。
        app.connectAndStart(pairingCode: null);
      },
    );
  }

  Widget _deviceTile(DiscoveredDevice d) {
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
  }

  void _showManualDialog(BuildContext context) {
    // DEBUG 模式下默认填充调试机地址，省去手敲。
    final ipCtrl = TextEditingController(text: DEBUG ? '10.31.30.41' : '');
    showDialog(
      context: context,
      builder: (_) => AlertDialog(
        title: const Text('手动输入 IP'),
        content: TextField(
          controller: ipCtrl,
          decoration: const InputDecoration(
            hintText: '192.168.1.10',
            labelText: '桌面端 IP',
          ),
          keyboardType: TextInputType.number,
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context),
            child: const Text('取消'),
          ),
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

class _SectionHeader extends StatelessWidget {
  final String text;
  const _SectionHeader(this.text);
  @override
  Widget build(BuildContext context) => ListTile(
    title: Text(text, style: Theme.of(context).textTheme.titleSmall),
  );
}

/// 调试：采集 PCM 转储开关。启用后下次采集会把原始 PCM + Opus 帧写到平台调试目录。
///
/// 初始值跟随 [DUMP_ENABLE]（即 [DEBUG]）；运行时仍可手动切换。
class _DumpPcmTile extends StatefulWidget {
  final AppState app;
  const _DumpPcmTile({required this.app});
  @override
  State<_DumpPcmTile> createState() => _DumpPcmTileState();
}

class _DumpPcmTileState extends State<_DumpPcmTile> {
  late bool _enabled = DUMP_ENABLE;
  @override
  Widget build(BuildContext context) {
    return SwitchListTile(
      secondary: const Icon(Icons.bug_report, size: 20),
      title: const Text('调试：保存采集 PCM', style: TextStyle(fontSize: 13)),
      subtitle: Text(
        _enabled
            ? '已启用：iOS 在 App Group，Android 在 Download/soundlink_dump/（失败回退私有目录）'
            : '关闭',
        style: const TextStyle(fontSize: 11),
      ),
      value: _enabled,
      onChanged: (v) async {
        await widget.app.platform.setDumpPcm(v);
        setState(() => _enabled = v);
      },
    );
  }
}
