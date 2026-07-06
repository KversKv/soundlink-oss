// SoundLink 移动端 App 根 + 全局状态。
//
// 架构：Flutter 主 App（配对/发现/设置/广播引导）+ 原生采集组件（不嵌入 Flutter）。
// 详见 docs/First/07-tech-stack.md §6、08-platform-notes.md §1b。

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';

import 'main.dart' show DUMP_ENABLE;
import 'src/constants.dart';
import 'src/models/connection_state.dart';
import 'src/models/device.dart';
import 'src/services/control_client.dart';
import 'src/services/discovery_service.dart';
import 'src/services/pairing_service.dart';
import 'src/services/platform_service.dart';
import 'src/services/trust_store.dart';
import 'src/pages/home_page.dart';

class SoundLinkApp extends StatelessWidget {
  const SoundLinkApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'SoundLink',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(seedColor: const Color(0xFF2E5BFF)),
        useMaterial3: true,
      ),
      home: const HomePage(),
    );
  }
}

/// 全局应用状态。
class AppState extends ChangeNotifier {
  final DiscoveryService discovery = DiscoveryService();
  final PlatformService platform = PlatformService();

  LinkState _conn = LinkState.disconnected;
  DiscoveredDevice? _selectedDevice;
  PairingService? _pairing;
  String? _lastError;
  List<DiscoveredDevice> _devices = [];
  bool _scanning = false;
  String _deviceId = '';
  String _identityPubB64 = '';
  int _jitterMs = defaultJitterMs;
  List<TrustedReceiver> _trusted = [];

  LinkState get conn => _conn;
  DiscoveredDevice? get selectedDevice => _selectedDevice;
  String? get lastError => _lastError;
  List<DiscoveredDevice> get devices => _devices;
  bool get scanning => _scanning;
  String get deviceId => _deviceId;
  String get identityPubB64 => _identityPubB64;
  int get jitterMs => _jitterMs;
  List<TrustedReceiver> get trustedReceivers => _trusted;

  AppState() {
    _init();
  }

  Future<void> _init() async {
    try {
      _deviceId = await platform.getDeviceId();
    } catch (_) {
      _deviceId = 'mobile-${DateTime.now().millisecondsSinceEpoch % 0x10000}';
    }
    try {
      final identity = await MobileIdentity.loadOrCreate();
      _deviceId = identity.deviceId;
      _identityPubB64 = identity.identityPubB64;
    } catch (e) {
      // 身份生成失败不阻塞，配对时 sender_identity_pub 留空。
      debugPrint('身份加载失败：$e');
    }
    // DUMP_ENABLE 默认开启时同步到原生侧（用户仍可在“设备发现”页手动关闭）。
    if (DUMP_ENABLE) {
      try {
        await platform.setDumpPcm(true);
      } catch (e) {
        debugPrint('启用 PCM 转储失败：$e');
      }
    }
    try {
      await refreshTrusted();
    } catch (e) {
      debugPrint('初始化信任存储失败：$e');
      _trusted = [];
      notifyListeners();
    }
  }

  Future<void> refreshTrusted() async {
    try {
      _trusted = await TrustStore.loadAll();
    } catch (e) {
      debugPrint('加载信任存储失败：$e');
      _trusted = [];
    }
    notifyListeners();
  }

  void setJitterMs(int v) {
    _jitterMs = v;
    notifyListeners();
  }

  /// 扫描局域网设备。
  Future<void> scan() async {
    if (_scanning) return;
    _scanning = true;
    _lastError = null;
    notifyListeners();
    try {
      _devices = await discovery.scan();
    } catch (e) {
      _lastError = '扫描失败：$e';
    }
    _scanning = false;
    notifyListeners();
  }

  void selectDevice(DiscoveredDevice? d) {
    _selectedDevice = d;
    notifyListeners();
  }

  /// 用配对码连接并启动采集。
  /// [pairingCode] 为 null 时走已信任路径（跳过配对码）。
  Future<void> connectAndStart({String? pairingCode}) async {
    final device = _selectedDevice;
    if (device == null) {
      _lastError = '请先选择设备';
      notifyListeners();
      return;
    }
    _lastError = null;
    _conn = LinkState.connecting;
    notifyListeners();

    final control = ControlClient(host: device.host, port: device.controlPort);
    _pairing = PairingService(
      control: control,
      platform: platform,
      deviceId: _deviceId,
      deviceName: 'SoundLink Mobile',
      platformName: _platformName(),
      identityPubB64: _identityPubB64,
    );
    try {
      await _pairing!.connectAndStart(
        device,
        pairingCode: pairingCode,
        onState: (s) {
          _conn = s;
          notifyListeners();
        },
      );
      await refreshTrusted();
    } catch (e) {
      _lastError = '$e';
      _conn = LinkState.error;
      notifyListeners();
    }
  }

  /// 移除已信任接收端。
  Future<void> removeTrusted(String deviceId) async {
    await TrustStore.remove(deviceId);
    await refreshTrusted();
  }

  /// 手动输入 IP 连接（兜底，无 mDNS 时）。
  void addManualDevice(String ip, {int? controlPort, int? audioPort}) {
    _selectedDevice = DiscoveredDevice(
      deviceId: 'manual-$ip',
      deviceName: ip,
      role: 'receiver',
      protocolVersion: protocolVersion,
      pairingRequired: true,
      audioCodec: 'opus',
      sampleRate: sampleRate,
      controlPort: controlPort ?? defaultControlPort,
      audioPort: audioPort ?? defaultAudioPort,
      host: ip,
    );
    _devices = [..._devices, _selectedDevice!];
    notifyListeners();
  }

  Future<void> stop() async {
    try {
      await _pairing?.stop();
    } catch (_) {}
    _conn = LinkState.disconnected;
    notifyListeners();
  }

  String _platformName() {
    if (defaultTargetPlatform == TargetPlatform.android) return 'android';
    return 'ios';
  }
}
