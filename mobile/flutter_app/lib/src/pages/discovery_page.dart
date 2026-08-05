// 设备页：扫描局域网桌面 Receiver，列表选择；支持手动输入 IP。
// 已信任设备可直接快速重连（跳过配对码）。
// 页面底部内嵌配对区块：输入配对码连接所选设备，或停止广播。
// 广播中自动隐藏扫描/设备列表/配对输入等连接相关内容，仅保留停止入口。

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';

import '../../app.dart';
import '../../main.dart' show DEBUG, DUMP_ENABLE;
import '../models/connection_state.dart';
import '../models/device.dart';
import '../services/trust_store.dart';

class DiscoveryPage extends StatefulWidget {
  final AppState app;
  const DiscoveryPage({super.key, required this.app});

  @override
  State<DiscoveryPage> createState() => _DiscoveryPageState();
}

class _DiscoveryPageState extends State<DiscoveryPage> {
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
      appBar: AppBar(title: const Text('设备')),
      body: ListenableBuilder(
        listenable: app,
        builder: (context, _) {
          final broadcasting = app.conn == LinkState.streaming;
          // 广播中：隐藏扫描/设备列表/配对输入等连接相关内容，仅保留停止入口。
          if (broadcasting) {
            return ListView(
              children: [
                if (app.lastError != null)
                  _banner(app.lastError!, Colors.red.shade50),
                _buildBroadcastingCard(context, app),
              ],
            );
          }
          return ListView(
            children: [
              if (app.lastError != null)
                _banner(app.lastError!, Colors.red.shade50),
              if (app.lastReceiver != null) ...[
                const _SectionHeader('上次连接'),
                _lastReceiverTile(context, app.lastReceiver!),
                const Divider(),
              ],
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
              const Divider(),
              _buildGuideSection(context),
              const Divider(),
              _buildPairingSection(context, app),
              // 调试：采集 PCM 转储开关
              _DumpPcmTile(app: app),
            ],
          );
        },
      ),
    );
  }

  /// 配对区块：显示目标设备、配对码输入与连接按钮（仅在未广播时显示）。
  Widget _buildPairingSection(BuildContext context, AppState app) {
    final device = app.selectedDevice;
    return Padding(
      padding: const EdgeInsets.all(20),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text('目标设备', style: Theme.of(context).textTheme.titleSmall),
          const SizedBox(height: 4),
          Text(device != null ? _deviceLabel(device) : '未选择（请在上方列表选择）'),
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
        ],
      ),
    );
  }

  /// 平台广播引导（原广播页内容精简并入，仅未广播时显示）。
  Widget _buildGuideSection(BuildContext context) {
    final isIOS = defaultTargetPlatform != TargetPlatform.android;
    final steps = isIOS
        ? const <String>[
            '打开系统「控制中心」（屏幕右上角下滑）',
            '长按「屏幕录制」按钮',
            '在列表中选择 SoundLink',
            '点击「开始广播」，音频将开始传输',
          ]
        : const <String>[
            '在下方选择设备并点击「配对并开始广播」',
            '系统将弹出「屏幕共享/录制」授权弹窗',
            '点击「立即开始」授权 MediaProjection',
            '通知栏将显示采集状态，音频开始传输',
          ];
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            isIOS ? 'iOS 开启广播步骤' : 'Android 开启采集步骤',
            style: Theme.of(context).textTheme.titleSmall,
          ),
          const SizedBox(height: 8),
          for (var i = 0; i < steps.length; i++)
            Padding(
              padding: const EdgeInsets.symmetric(vertical: 2),
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text('${i + 1}. ', style: const TextStyle(fontSize: 13)),
                  Expanded(
                    child: Text(steps[i], style: const TextStyle(fontSize: 13)),
                  ),
                ],
              ),
            ),
          const SizedBox(height: 8),
          Text(
            '说明：基于系统官方采集能力，支持大部分普通应用音频；受 DRM 或应用限制的内容可能无法采集。'
            '${isIOS ? " iOS 不支持应用静默修改全局媒体音量，如本机仍外放请用系统音量/耳机控制。" : ""}',
            style: TextStyle(fontSize: 12, color: Colors.grey.shade700),
          ),
        ],
      ),
    );
  }

  /// 广播中卡片：隐藏连接相关内容后，仅展示状态与停止入口。
  Widget _buildBroadcastingCard(BuildContext context, AppState app) {
    return Padding(
      padding: const EdgeInsets.all(20),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Card(
            color: Colors.green.shade50,
            child: Padding(
              padding: const EdgeInsets.all(16),
              child: Row(
                children: [
                  const Icon(Icons.cast_connected, color: Colors.green),
                  const SizedBox(width: 12),
                  Expanded(
                    child: Text(
                      '正在广播音频到 ${app.selectedDevice?.deviceName ?? ""}',
                      style: const TextStyle(fontSize: 14),
                    ),
                  ),
                ],
              ),
            ),
          ),
          const SizedBox(height: 20),
          FilledButton.tonalIcon(
            onPressed: _busy ? null : () => app.stop(),
            icon: const Icon(Icons.stop),
            label: const Text('停止广播'),
            style: FilledButton.styleFrom(
              backgroundColor: Colors.red.shade100,
            ),
          ),
        ],
      ),
    );
  }

  /// 目标设备显示文案：手动添加的设备名称即 IP，避免名称与 host 重复成两行相同 IP。
  static String _deviceLabel(DiscoveredDevice d) {
    if (d.deviceName == d.host) return d.host;
    return '${d.deviceName}\n${d.host}';
  }

  Future<void> _connect(AppState app, {required bool trusted}) async {
    setState(() => _busy = true);
    try {
      await app.connectAndStart(pairingCode: trusted ? null : _codeCtrl.text);
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Widget _lastReceiverTile(BuildContext context, DiscoveredDevice d) {
    return ListTile(
      leading: const Icon(Icons.history, color: Colors.blue),
      title: Text(d.deviceName),
      subtitle: Text('${d.host}  ·  上次连接设备'),
      trailing: const Icon(Icons.play_arrow),
      onTap: () {
        widget.app.selectDevice(d);
        widget.app.connectAndStart(pairingCode: null);
      },
    );
  }

  Widget _trustedTile(BuildContext context, TrustedReceiver t) {
    return ListTile(
      leading: const Icon(Icons.verified, color: Colors.green),
      title: Text(t.deviceName),
      subtitle: Text('${t.host}  ·  已信任'),
      trailing: IconButton(
        icon: const Icon(Icons.delete_outline, size: 20),
        onPressed: () => widget.app.removeTrusted(t.deviceId),
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
        widget.app.selectDevice(device);
        // 已信任设备直接连接（无配对码）。
        widget.app.connectAndStart(pairingCode: null);
      },
    );
  }

  Widget _deviceTile(DiscoveredDevice d) {
    final app = widget.app;
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
    final app = widget.app;
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
          // IPv4 含 "."，纯数字键盘无法输入该符号；
          // numberWithOptions(decimal: true) 在 iOS/Android 上都会提供 "." 键。
          keyboardType: const TextInputType.numberWithOptions(decimal: true),
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
