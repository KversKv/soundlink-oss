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
          final audio = app.audioSettings;
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
              _OptionTile(
                title: '采样率（当前版本固定）',
                value: audio.sampleRate,
                values: audioSampleRateOptions,
                label: (v) => '$v Hz',
                onChanged: (v) =>
                    app.setAudioSettings(audio.copyWith(sampleRate: v)),
              ),
              _OptionTile(
                title: '声道（当前版本固定）',
                value: audio.channels,
                values: audioChannelOptions,
                label: (v) => v == 1 ? 'Mono' : 'Stereo',
                onChanged: (v) =>
                    app.setAudioSettings(audio.copyWith(channels: v)),
              ),
              _OptionTile(
                title: '帧长（当前版本固定）',
                value: audio.frameDurationMs,
                values: audioFrameDurationOptions,
                label: (v) => '$v ms',
                onChanged: (v) =>
                    app.setAudioSettings(audio.copyWith(frameDurationMs: v)),
              ),
              _OptionTile(
                title: 'Opus 码率',
                value: audio.bitrate,
                values: audioBitrateOptions,
                label: (v) => '${v ~/ 1000} kbps',
                onChanged: (v) =>
                    app.setAudioSettings(audio.copyWith(bitrate: v)),
              ),
              Padding(
                padding: const EdgeInsets.symmetric(
                  horizontal: 16,
                  vertical: 8,
                ),
                child: FilledButton.icon(
                  onPressed: () async {
                    final messenger = ScaffoldMessenger.of(context);
                    final result = await app.autoDetectAudioSettings();
                    if (!context.mounted) return;
                    await showDialog<void>(
                      context: context,
                      builder: (context) => AlertDialog(
                        title: const Text('自动探测结果'),
                        content: Text(_recommendationText(result)),
                        actions: [
                          TextButton(
                            onPressed: () => Navigator.of(context).pop(),
                            child: const Text('知道了'),
                          ),
                        ],
                      ),
                    );
                    if (!context.mounted) return;
                    if (result.pausedStream) {
                      messenger.showSnackBar(
                        const SnackBar(content: Text('探测时已暂停音频流，请返回首页重新开始广播。')),
                      );
                    }
                  },
                  icon: const Icon(Icons.network_check),
                  label: const Text('自动探测并推荐参数'),
                ),
              ),
              const Padding(
                padding: EdgeInsets.fromLTRB(16, 0, 16, 12),
                child: Text(
                  '设置会保存到本 App 数据中；覆盖安装、热重启通常保留，卸载或清除数据会删除。当前版本真正生效：Opus 码率、Jitter；采样率/声道/帧长固定为 48kHz/Stereo/10ms。',
                  style: TextStyle(fontSize: 12, color: Colors.black54),
                ),
              ),
              const _SectionHeader('设备'),
              ListTile(
                title: const Text('本机 Device ID'),
                subtitle: Text(app.deviceId.isEmpty ? '获取中…' : app.deviceId),
              ),
              if (app.lastReceiver != null)
                ListTile(
                  title: const Text('上次设备'),
                  subtitle: Text(
                    '${app.lastReceiver!.deviceName} · ${app.lastReceiver!.host}',
                  ),
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

String _recommendationText(AudioRecommendation result) {
  final s = result.settings;
  // O4：展示真实 UDP 音频面指标（丢包/抖动）。
  final metric = result.lossRate == null
      ? '未获得有效样本'
      : '丢包 ${(result.lossRate! * 100).toStringAsFixed(1)}%，抖动 ${result.jitterMs ?? 0} ms';
  return [
    result.reason,
    '',
    '音频面指标：$metric',
    '推荐参数：${s.sampleRate} Hz / ${s.channels == 1 ? 'Mono' : 'Stereo'} / ${s.frameDurationMs} ms / ${s.bitrate ~/ 1000} kbps / Jitter ${s.jitterMs} ms',
    '探测基于接收端真实统计，无需暂停当前广播。',
  ].join('\n');
}

class _OptionTile extends StatelessWidget {
  final String title;
  final int value;
  final List<int> values;
  final String Function(int) label;
  final ValueChanged<int> onChanged;

  const _OptionTile({
    required this.title,
    required this.value,
    required this.values,
    required this.label,
    required this.onChanged,
  });

  @override
  Widget build(BuildContext context) {
    return ListTile(
      title: Text(title),
      trailing: DropdownButton<int>(
        value: value,
        items: values
            .map((v) => DropdownMenuItem<int>(value: v, child: Text(label(v))))
            .toList(),
        onChanged: (v) {
          if (v != null) onChanged(v);
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
