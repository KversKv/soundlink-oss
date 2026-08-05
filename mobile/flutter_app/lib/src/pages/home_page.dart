// 主页：底部导航（设备 / 设置）。
// 设备页内嵌配对区块与平台广播引导（原配对页/广播页已合并）。
//
// 创建 AppState 并显式注入各子页面，避免全局单例与 context 查找。

import 'package:flutter/material.dart';

import '../../app.dart';
import '../models/connection_state.dart';
import 'discovery_page.dart';
import 'settings_page.dart';

class HomePage extends StatefulWidget {
  const HomePage({super.key});

  @override
  State<HomePage> createState() => _HomePageState();
}

class _HomePageState extends State<HomePage> {
  int _index = 0;
  late final AppState _app;

  @override
  void initState() {
    super.initState();
    _app = AppState();
    // 进入即开始扫描。
    WidgetsBinding.instance.addPostFrameCallback((_) => _app.scan());
  }

  @override
  void dispose() {
    _app.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: _app,
      builder: (context, _) {
        return Scaffold(
          body: Column(
            children: [
              StatusBar(_app),
              Expanded(
                child: IndexedStack(
                  index: _index,
                  children: [
                    DiscoveryPage(app: _app),
                    SettingsPage(app: _app),
                  ],
                ),
              ),
            ],
          ),
          bottomNavigationBar: NavigationBar(
            selectedIndex: _index,
            onDestinationSelected: (i) => setState(() => _index = i),
            destinations: const [
              NavigationDestination(
                icon: Icon(Icons.devices_outlined),
                selectedIcon: Icon(Icons.devices),
                label: '设备',
              ),
              NavigationDestination(
                icon: Icon(Icons.settings_outlined),
                selectedIcon: Icon(Icons.settings),
                label: '设置',
              ),
            ],
          ),
        );
      },
    );
  }
}

/// 顶部状态条（由各页面顶部嵌入）。
class StatusBar extends StatelessWidget {
  final AppState app;
  const StatusBar(this.app, {super.key});

  @override
  Widget build(BuildContext context) {
    final conn = app.conn;
    return Container(
      width: double.infinity,
      color: _statusColor(conn),
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 6),
      child: SafeArea(
        bottom: false,
        child: Text(
          '状态：${conn.label}'
          '${app.selectedDevice != null ? "  ·  ${app.selectedDevice!.deviceName}" : ""}',
          style: const TextStyle(color: Colors.white, fontSize: 13),
        ),
      ),
    );
  }

  Color _statusColor(LinkState c) => switch (c) {
    LinkState.streaming => Colors.green.shade700,
    LinkState.connecting || LinkState.pairing => Colors.orange.shade700,
    LinkState.reconnecting || LinkState.error => Colors.red.shade700,
    LinkState.connected || LinkState.paired => Colors.blue.shade700,
    _ => Colors.grey.shade700,
  };
}
