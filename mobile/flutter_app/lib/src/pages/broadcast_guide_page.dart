// 广播引导页：分平台引导用户开启系统广播（iOS ReplayKit / Android MediaProjection）。

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';

import '../../app.dart';
import '../models/connection_state.dart';

class BroadcastGuidePage extends StatelessWidget {
  final AppState app;
  const BroadcastGuidePage({super.key, required this.app});

  @override
  Widget build(BuildContext context) {
    final isIOS = defaultTargetPlatform != TargetPlatform.android;
    return Scaffold(
      appBar: AppBar(title: const Text('广播引导')),
      body: ListenableBuilder(
        listenable: app,
        builder: (context, _) {
          return ListView(
            padding: const EdgeInsets.all(20),
            children: [
              _statusCard(app),
              const SizedBox(height: 16),
              if (isIOS) ..._iosSteps(context) else ..._androidSteps(context),
              const SizedBox(height: 16),
              const Card(
                child: Padding(
                  padding: EdgeInsets.all(12),
                  child: Text(
                    '说明：本软件基于系统官方采集能力，支持大部分普通应用音频；'
                    '受 DRM 或应用限制的内容可能无法采集。',
                    style: TextStyle(fontSize: 13),
                  ),
                ),
              ),
            ],
          );
        },
      ),
    );
  }

  Widget _statusCard(AppState app) => Card(
        color: app.conn == LinkState.streaming
            ? Colors.green.shade50
            : Colors.grey.shade100,
        child: Padding(
          padding: const EdgeInsets.all(12),
          child: Row(
            children: [
              Icon(app.conn == LinkState.streaming
                  ? Icons.cast_connected
                  : Icons.cast),
              const SizedBox(width: 12),
              Expanded(
                child: Text(
                  app.conn == LinkState.streaming
                      ? '正在广播音频到 ${app.selectedDevice?.deviceName ?? ""}'
                      : '尚未开始广播。请先在“配对”页连接。',
                ),
              ),
            ],
          ),
        ),
      );

  List<Widget> _iosSteps(BuildContext context) => const [
        Text('iOS 开启广播步骤', style: TextStyle(fontWeight: FontWeight.bold)),
        SizedBox(height: 8),
        ListTile(leading: Text('1'), title: Text('打开系统“控制中心”（屏幕右上角下滑）')),
        ListTile(leading: Text('2'), title: Text('长按“屏幕录制”按钮')),
        ListTile(leading: Text('3'), title: Text('在列表中选择 SoundLink')),
        ListTile(leading: Text('4'), title: Text('点击“开始广播”，音频将开始传输')),
      ];

  List<Widget> _androidSteps(BuildContext context) => const [
        Text('Android 开启采集步骤', style: TextStyle(fontWeight: FontWeight.bold)),
        SizedBox(height: 8),
        ListTile(leading: Text('1'), title: Text('在“配对”页点击“配对并开始广播”')),
        ListTile(leading: Text('2'), title: Text('系统将弹出“屏幕共享/录制”授权弹窗')),
        ListTile(leading: Text('3'), title: Text('点击“立即开始”授权 MediaProjection')),
        ListTile(leading: Text('4'), title: Text('通知栏将显示采集状态，音频开始传输')),
        ListTile(
            leading: Text('!'),
            title: Text('部分应用可拒绝被采集；受保护内容不可采（系统限制）')),
      ];
}
