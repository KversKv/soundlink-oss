// 设置页：Jitter 模式、音频参数展示、设备信息。

import 'package:flutter/material.dart';

import '../../app.dart';
import '../constants.dart';

class SettingsPage extends StatelessWidget {
  final AppState app;
  const SettingsPage({super.key, required this.app});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('设置')),
      body: ListenableBuilder(
        listenable: app,
        builder: (context, _) {
          return ListView(
            children: [
              const _SectionHeader('Jitter 缓冲（影响延迟/稳定性）'),
              RadioGroup<int>(
                groupValue: app.jitterMs,
                onChanged: (v) => app.setJitterMs(v ?? jitterBalancedMs),
                child: Column(
                  children: [
                    RadioListTile<int>(
                      value: jitterLowMs,
                      title: const Text('低延迟（40ms）'),
                      subtitle: const Text('网络稳定时'),
                    ),
                    RadioListTile<int>(
                      value: jitterBalancedMs,
                      title: const Text('平衡（80ms，默认）'),
                    ),
                    RadioListTile<int>(
                      value: jitterStableMs,
                      title: const Text('稳定（150ms）'),
                      subtitle: const Text('Wi-Fi 较差时'),
                    ),
                  ],
                ),
              ),
              const _SectionHeader('音频参数'),
              ListTile(
                title: const Text('采样率'),
                trailing: Text('$sampleRate Hz'),
              ),
              ListTile(
                title: const Text('声道'),
                trailing: Text('$channels 立体声'),
              ),
              ListTile(
                title: const Text('编码'),
                trailing: Text(
                  'Opus ${frameDurationMs}ms / ${opusBitrate ~/ 1000}kbps',
                ),
              ),
              const _SectionHeader('设备'),
              ListTile(
                title: const Text('本机 Device ID'),
                subtitle: Text(app.deviceId.isEmpty ? '获取中…' : app.deviceId),
              ),
              const _SectionHeader('关于'),
              const ListTile(
                title: Text('SoundLink'),
                subtitle: Text('局域网音频流转 · 发送端'),
              ),
              ListTile(
                title: const Text('协议版本'),
                trailing: Text('v$protocolVersion'),
              ),
            ],
          );
        },
      ),
    );
  }
}

class _SectionHeader extends StatelessWidget {
  final String text;
  const _SectionHeader(this.text);
  @override
  Widget build(BuildContext context) => ListTile(
    title: Text(text, style: Theme.of(context).textTheme.titleSmall),
  );
}
