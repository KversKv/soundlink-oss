// SoundLink 移动端 App 根 + 全局状态。
//
// 架构：Flutter 主 App（配对/发现/设置/广播引导）+ 原生采集组件（不嵌入 Flutter）。
// 详见 docs/First/07-tech-stack.md §6、08-platform-notes.md §1b。

import 'dart:async';
import 'dart:io';

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

class AudioRecommendation {
  final AudioSettings settings;
  final bool pausedStream;
  final int sampleCount;
  final double? avgLatencyMs;
  final double? maxLatencyMs;
  final String reason;

  const AudioRecommendation({
    required this.settings,
    required this.pausedStream,
    required this.sampleCount,
    required this.avgLatencyMs,
    required this.maxLatencyMs,
    required this.reason,
  });
}

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
  AudioSettings _audioSettings = AudioSettings.defaults();
  DiscoveredDevice? _lastReceiver;
  List<TrustedReceiver> _trusted = [];

  LinkState get conn => _conn;
  DiscoveredDevice? get selectedDevice => _selectedDevice;
  String? get lastError => _lastError;
  List<DiscoveredDevice> get devices => _devices;
  bool get scanning => _scanning;
  String get deviceId => _deviceId;
  String get identityPubB64 => _identityPubB64;
  int get jitterMs => _jitterMs;
  AudioSettings get audioSettings => _audioSettings;
  DiscoveredDevice? get lastReceiver => _lastReceiver;
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
      _audioSettings = await TrustStore.loadAudioSettings();
      _jitterMs = _audioSettings.jitterMs;
      _lastReceiver = await TrustStore.loadLastReceiver();
      _selectedDevice = _lastReceiver;
    } catch (e) {
      debugPrint('初始化本地设置失败：$e');
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

  Future<void> setJitterMs(int v) async {
    await setAudioSettings(_audioSettings.copyWith(jitterMs: v));
  }

  Future<void> setAudioSettings(AudioSettings settings) async {
    _audioSettings = settings.normalized();
    _jitterMs = _audioSettings.jitterMs;
    await TrustStore.saveAudioSettings(_audioSettings);
    _pairing?.sendAudioParamsUpdate(_audioSettings);
    notifyListeners();
  }

  Future<AudioRecommendation> autoDetectAudioSettings() async {
    final wasStreaming = _conn == LinkState.streaming;
    final device = _selectedDevice ?? _lastReceiver;
    if (wasStreaming) {
      await stop();
      await Future<void>.delayed(const Duration(milliseconds: 300));
    }

    final samples = device == null
        ? <Duration>[]
        : await _probeControlLatency(device.host, device.controlPort);
    final avgMs = samples.isEmpty
        ? null
        : samples.map((d) => d.inMilliseconds).reduce((a, b) => a + b) /
              samples.length;
    final maxMs = samples.isEmpty
        ? null
        : samples
              .map((d) => d.inMilliseconds.toDouble())
              .reduce((a, b) => a > b ? a : b);
    final spreadMs = avgMs == null || maxMs == null ? null : maxMs - avgMs;

    final current = _audioSettings.normalized();
    final recommended = _recommendAudioSettings(current, avgMs, spreadMs);
    await setAudioSettings(recommended);
    return AudioRecommendation(
      settings: recommended,
      pausedStream: wasStreaming,
      sampleCount: samples.length,
      avgLatencyMs: avgMs,
      maxLatencyMs: maxMs,
      reason: _recommendationReason(avgMs, spreadMs, samples.length),
    );
  }

  AudioSettings _recommendAudioSettings(
    AudioSettings current,
    double? avgMs,
    double? spreadMs,
  ) {
    if (avgMs == null || spreadMs == null) {
      return current.copyWith(bitrate: opusBitrate, jitterMs: defaultJitterMs);
    }
    if (avgMs >= 80 || spreadMs >= 35) {
      return current.copyWith(bitrate: 96000, jitterMs: jitterStableMs);
    }
    if (avgMs <= 25 && spreadMs <= 10) {
      return current.copyWith(bitrate: 160000, jitterMs: jitterLowMs);
    }
    return current.copyWith(bitrate: 128000, jitterMs: jitterBalancedMs);
  }

  String _recommendationReason(
    double? avgMs,
    double? spreadMs,
    int sampleCount,
  ) {
    if (sampleCount == 0 || avgMs == null || spreadMs == null) {
      return '未连接到可探测的接收端，已使用默认稳定参数。';
    }
    if (avgMs >= 80 || spreadMs >= 35) {
      return '控制链路延迟或波动较高，推荐降低码率并提高 Jitter 稳定性。';
    }
    if (avgMs <= 25 && spreadMs <= 10) {
      return '控制链路延迟低且波动小，推荐更高码率和低延迟 Jitter。';
    }
    return '控制链路质量中等，推荐平衡参数。';
  }

  Future<List<Duration>> _probeControlLatency(String host, int port) async {
    final samples = <Duration>[];
    for (var i = 0; i < 5; i++) {
      final sw = Stopwatch()..start();
      try {
        final socket = await Socket.connect(
          host,
          port,
          timeout: const Duration(milliseconds: 800),
        );
        sw.stop();
        samples.add(sw.elapsed);
        socket.destroy();
      } catch (_) {
        sw.stop();
      }
      await Future<void>.delayed(const Duration(milliseconds: 80));
    }
    return samples;
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
        audioSettings: _audioSettings,
        onState: (s) {
          if (s == LinkState.reconnecting) {
            // 仅在远端主动 stream_stop / error 或重连失败时进入此分支。
            // 尝试读取 Extension 写入的停止原因，便于排查"直播已停止"。
            _conn = LinkState.disconnected;
            _pairing = null;
            _loadStopReason();
          } else {
            _conn = s;
          }
          notifyListeners();
        },
      );
      _lastReceiver = device;
      await TrustStore.saveLastReceiver(device);
      await refreshTrusted();
    } catch (e) {
      try {
        await _pairing?.stop();
      } catch (_) {}
      _pairing = null;
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
    _pairing = null;
    _conn = LinkState.disconnected;
    notifyListeners();
  }

  /// 读取 Broadcast Extension 停止原因并更新错误提示。
  Future<void> _loadStopReason() async {
    try {
      final info = await platform.popStopReason();
      if (info != null) {
        final reason = info['reason'] as String? ?? '';
        _lastError = reason.isNotEmpty
            ? '直播已停止：$reason'
            : '对端已停止或控制连接已断开，已自动停止采集';
      } else {
        _lastError = '对端已停止或控制连接已断开，已自动停止采集';
      }
    } catch (_) {
      _lastError = '对端已停止或控制连接已断开，已自动停止采集';
    }
    notifyListeners();
  }

  String _platformName() {
    if (defaultTargetPlatform == TargetPlatform.android) return 'android';
    return 'ios';
  }
}
