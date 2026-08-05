// 配对与连接编排（Sender 视角状态机，spec §6.1）。
//
// 流程：hello → hello_ack → (pair_request → pair_response) → stream_start → ack
// → 通知原生采集组件开始广播（经 PlatformService）。
//
// 派生 audio_key 后写入 App Group / Service 共享配置，供原生采集组件读取。

import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';

import '../constants.dart';
import '../crypto/pairing.dart';
import '../models/connection_state.dart';
import '../models/device.dart';
import '../protocol/control_message.dart';
import 'control_client.dart';
import 'platform_service.dart';
import 'trust_store.dart';

class PairingService {
  final ControlClient control;
  final PlatformService platform;
  final String deviceId;
  final String deviceName;
  final String platformName; // "ios" / "android"
  final String identityPubB64; // 本机 Ed25519 公钥（base64）

  SessionKeys? _keys;
  StreamSubscription<void>? _disconnectSub;
  StreamSubscription<Map<String, dynamic>>? _messageSub;
  Timer? _heartbeatTimer;
  Timer? _statsTimer;
  final int _streamId = defaultStreamId;
  int _audioPort = defaultAudioPort;
  DiscoveredDevice? _device;
  AudioSettings? _audioSettings;
  void Function(LinkState)? _onState;
  bool _stoppedByUser = false;
  bool _streamStopFromRemote = false;
  bool _reconnecting = false;

  PairingService({
    required this.control,
    required this.platform,
    required this.deviceId,
    required this.deviceName,
    required this.platformName,
    required this.identityPubB64,
  });

  SessionKeys? get sessionKeys => _keys;
  bool get isReconnecting => _reconnecting;

  /// 执行完整握手与流启动。
  ///
  /// [pairingCode] 配对码（8 位数字）。已信任设备传 null（hello.trusted 路径）。
  /// [onState] 状态变更回调。
  Future<void> connectAndStart(
    DiscoveredDevice device, {
    String? pairingCode,
    required AudioSettings audioSettings,
    required void Function(LinkState) onState,
  }) async {
    _device = device;
    _audioSettings = audioSettings;
    _onState = onState;
    _stoppedByUser = false;
    _streamStopFromRemote = false;
    _reconnecting = false;
    onState(LinkState.connecting);
    await control.connect();
    _wireControlEvents();
    await _handshakeAndStream(device, pairingCode: pairingCode);

    // 将会话配置写入 App Group / Service 共享，通知原生开始采集。
    final config = SessionConfig(
      targetHost: device.host,
      audioPort: _audioPort,
      streamId: _streamId,
      audioKey: _keys!.audioKey,
      sampleRate: audioSettings.sampleRate,
      channels: audioSettings.channels,
      frameDurationMs: audioSettings.frameDurationMs,
      bitrate: audioSettings.bitrate,
    );
    await platform.writeSessionConfig(config);
    await platform.startCapture();
    _startEventLoops();

    // N3：以本次流起始码率为基准，后续自适应在此基础上节流调整。
    _currentBitrate = audioSettings.bitrate;
    _lastBitrateAdjustMs = 0;

    onState(LinkState.streaming);
  }

  /// 建立/重建设备级握手并启动流（首次与重连复用）。
  ///
  /// 重连时若已存在 [_keys]（已信任会话）则跳过配对，直接 hello → pair_request
  /// （trusted 路径）→ stream_start。原 audio_key 不变，Extension 无需热切换。
  Future<void> _handshakeAndStream(
    DiscoveredDevice device, {
    String? pairingCode,
  }) async {
    final audioSettings = _audioSettings!;

    // 1) hello
    control.send(
      HelloMsg(
        msgId: newMsgId('c'),
        ts: nowMs(),
        protocolVersion: protocolVersion,
        deviceId: deviceId,
        deviceName: deviceName,
        role: 'sender',
        platform: platformName,
        capabilities: Capabilities(
          codec: ['opus'],
          sampleRate: audioSettings.sampleRate,
          channels: audioSettings.channels,
        ),
      ),
    );
    final helloAck = await control.waitFor((m) => m['type'] == 'hello_ack');
    _onState?.call(LinkState.connected);

    final receiverDeviceId = helloAck['device_id'] as String;
    final pairingRequired = (helloAck['pairing_required'] as bool?) ?? true;
    final trusted = (helloAck['trusted'] as bool?) ?? false;

    // 2) 配对（如需且未信任）
    if (pairingRequired && !trusted) {
      if (pairingCode == null) {
        throw StateError('需要配对码但未提供');
      }
      _onState?.call(LinkState.pairing);
      final handshake = await SenderHandshake.begin(
        pairingCode,
        receiverDeviceId,
      );
      final proof = await handshake.computeProof(receiverDeviceId);

      control.send(
        PairRequestMsg(
          msgId: newMsgId('c'),
          ts: nowMs(),
          deviceId: deviceId,
          senderPub: base64Encode(handshake.keyPair.publicKey),
          senderIdentityPub: identityPubB64,
          proof: base64Encode(proof),
        ),
      );

      final pairResp = await control.waitFor(
        (m) => m['type'] == 'pair_response',
      );
      if (pairResp['result'] != 'ok') {
        _onState?.call(LinkState.error);
        throw StateError('配对失败：${pairResp['error']}');
      }
      final receiverPub = base64Decode(pairResp['receiver_pub'] as String);
      final receiverIdentityPubB64 =
          pairResp['receiver_identity_pub'] as String? ?? '';
      final receiverProofB64 = pairResp['proof'] as String?;
      if (receiverProofB64 != null) {
        final ok = verifyReceiverProof(
          handshake.pairingSecret,
          receiverPub,
          handshake.keyPair.publicKey,
          receiverDeviceId,
          base64Decode(receiverProofB64),
        );
        if (!await ok) {
          _onState?.call(LinkState.error);
          throw StateError('Receiver 回证校验失败');
        }
      }
      _keys = await handshake.complete(receiverPub);

      // 保存信任关系（移动端信任接收端）。
      await TrustStore.add(
        TrustedReceiver(
          deviceId: receiverDeviceId,
          identityPubB64: receiverIdentityPubB64,
          deviceName: (helloAck['device_name'] as String?) ?? device.deviceName,
          host: device.host,
          controlPort: device.controlPort,
          audioPort: device.audioPort,
          lastSeen: DateTime.now().millisecondsSinceEpoch ~/ 1000,
        ),
      );
      _onState?.call(LinkState.paired);
    } else {
      // 已信任：跳过配对码，直接 X25519 协商（pairing_secret 用全 0 占位）。
      _onState?.call(LinkState.pairing);
      final handshake = await SenderHandshake.beginTrusted(receiverDeviceId);
      control.send(
        PairRequestMsg(
          msgId: newMsgId('c'),
          ts: nowMs(),
          deviceId: deviceId,
          senderPub: base64Encode(handshake.keyPair.publicKey),
          senderIdentityPub: identityPubB64,
          proof: '',
        ),
      );
      final pairResp = await control.waitFor(
        (m) => m['type'] == 'pair_response',
      );
      if (pairResp['result'] != 'ok') {
        throw StateError('协商失败：${pairResp['error']}');
      }
      final receiverPub = base64Decode(pairResp['receiver_pub'] as String);
      _keys = await handshake.complete(receiverPub);
      _onState?.call(LinkState.paired);
    }

    // 3) stream_start
    _audioPort = device.audioPort;
    control.send(
      StreamStartMsg(
        msgId: newMsgId('c'),
        ts: nowMs(),
        streamId: _streamId,
        audioPort: _audioPort,
        codec: 'opus',
        sampleRate: audioSettings.sampleRate,
        channels: audioSettings.channels,
        frameDurationMs: audioSettings.frameDurationMs,
        bitrate: audioSettings.bitrate,
      ),
    );
    final ack = await control.waitFor((m) => m['type'] == 'stream_start_ack');
    if (ack['result'] != 'ok') {
      throw StateError('stream_start 被拒绝：${ack['error']}');
    }
    _audioPort = (ack['receiver_audio_port'] as int?) ?? _audioPort;
  }

  /// 绑定控制通道断开与入站消息事件。
  void _wireControlEvents() {
    _disconnectSub?.cancel();
    _disconnectSub = control.onDisconnected.listen((_) async {
      // iOS 锁屏后主 App 被挂起，TCP 控制连接可能超时断开。
      // 但 BroadcastExtension 进程独立运行，不应因控制连接断开而停止。
      // 尝试自动重连：保持原 audio_key，Extension 无需热切换。
      if (_stoppedByUser || _streamStopFromRemote) return;
      await _tryReconnect();
    });
    _messageSub?.cancel();
    _messageSub = control.messages.listen((msg) async {
      if (msg['type'] == 'stream_stop' || msg['type'] == 'error') {
        _streamStopFromRemote = true;
        await _stopLocalCapture(clearSession: true);
        _onState?.call(LinkState.reconnecting);
      } else if (msg['type'] == 'stats') {
        _onReceiverStats(msg);
      }
    });
  }

  // N3：码率自适应状态（接收端建议值 → 归档 → 节流下发到原生 encoder）。
  int _currentBitrate = 0;
  int _lastBitrateAdjustMs = 0;
  // 码率自适应开关（由 App 层注入；对应桌面 jitter_mode=="auto" 的语义）。
  bool bitrateAdaptive = false;

  /// 接收端 stats 回传：自适应开启时把 recommended_bitrate 归档到
  /// bitrateStep 倍数并节流下发（最短间隔 bitrateAdjustMinIntervalMs）。
  void _onReceiverStats(Map<String, dynamic> msg) {
    if (!bitrateAdaptive) return;
    final rec = (msg['recommended_bitrate'] as num?)?.toInt() ?? 0;
    if (rec <= 0) return; // 0 = 样本不足，忽略
    final snapped = (rec / bitrateStep).round() * bitrateStep;
    if (snapped == _currentBitrate) return;
    final now = DateTime.now().millisecondsSinceEpoch;
    if (now - _lastBitrateAdjustMs < bitrateAdjustMinIntervalMs) return;
    _currentBitrate = snapped;
    _lastBitrateAdjustMs = now;
    platform.setBitrate(snapped);
  }

  /// 尝试自动重连：使用原设备信息与 audio_key 重建控制会话。
  /// 成功则恢复心跳/stats 并保持 LinkState.streaming；
  /// 失败才进入 LinkState.reconnecting 让用户感知。
  Future<void> _tryReconnect() async {
    if (_reconnecting) return;
    _reconnecting = true;
    _stopEventLoops();
    final device = _device;
    if (device == null) {
      _reconnecting = false;
      _onState?.call(LinkState.reconnecting);
      return;
    }
    _onState?.call(LinkState.connecting);

    var backoff = reconnectBackoffInitialMs;
    for (var attempt = 1; attempt <= reconnectMaxAttempts; attempt++) {
      if (_stoppedByUser || _streamStopFromRemote) {
        _reconnecting = false;
        return;
      }
      try {
        await control.connect();
        _wireControlEvents();
        await _handshakeAndStream(device);
        _startEventLoops();
        _reconnecting = false;
        _onState?.call(LinkState.streaming);
        return;
      } catch (e) {
        if (attempt == reconnectMaxAttempts) break;
        await Future<void>.delayed(Duration(milliseconds: backoff));
        backoff = (backoff * 2)
            .clamp(reconnectBackoffInitialMs, reconnectBackoffMaxMs)
            .toInt();
      }
    }
    _reconnecting = false;
    _onState?.call(LinkState.reconnecting);
  }

  /// O4：向接收端发起真实探测，等待 probe_result（基于接收端 UDP 音频面统计）。
  /// 返回 null 表示未连接或超时。probe_result 是 control_action 且 action=probe_result。
  Future<Map<String, dynamic>?> probeAudioParams({
    Duration timeout = const Duration(seconds: 3),
  }) async {
    if (!control.isConnected) return null;
    final msgId = newMsgId('c-probe');
    final future = control.messages
        .firstWhere(
          (m) =>
              m['type'] == 'control_action' &&
              m['action'] == ControlActions.audioParamsProbeResult &&
              m['reply_to'] == msgId,
        )
        .timeout(timeout, onTimeout: () => <String, dynamic>{});
    control.send(
      ControlActionMsg(
        msgId: msgId,
        ts: nowMs(),
        action: ControlActions.audioParamsProbeRequest,
        target: 'receiver',
      ),
    );
    final msg = await future;
    if (msg.isEmpty) return null;
    final payload = msg['payload'];
    return payload is Map<String, dynamic> ? payload : null;
  }

  void sendAudioParamsUpdate(AudioSettings settings) {
    if (!control.isConnected) return;
    control.send(
      ControlActionMsg(
        msgId: newMsgId('c-audio'),
        ts: nowMs(),
        action: ControlActions.audioParamsUpdate,
        target: 'receiver',
        payload: settings.normalized().toJson(),
      ),
    );
  }

  void _startEventLoops() {
    _stopEventLoops();
    _heartbeatTimer = Timer.periodic(
      const Duration(seconds: heartbeatIntervalSecs),
      (_) {
        if (!control.isConnected) return;
        control.send(HeartbeatMsg(msgId: newMsgId('c-hb'), ts: nowMs()));
      },
    );
    _statsTimer = Timer.periodic(const Duration(seconds: 1), (_) {
      if (!control.isConnected) return;
      control.send(
        StatsMsg(msgId: newMsgId('c-stats'), ts: nowMs(), streamId: _streamId),
      );
    });
  }

  void _stopEventLoops() {
    _heartbeatTimer?.cancel();
    _statsTimer?.cancel();
    _heartbeatTimer = null;
    _statsTimer = null;
  }

  Future<void> _stopLocalCapture({bool clearSession = false}) async {
    _stopEventLoops();
    await platform.stopCapture(clearSession: clearSession);
    _keys = null;
  }

  /// 停止流并断开。
  Future<void> stop() async {
    _stoppedByUser = true;
    _stopEventLoops();
    if (control.isConnected) {
      await control.sendAndFlush(
        StreamStopMsg(msgId: newMsgId('c'), ts: nowMs(), streamId: _streamId),
      );
    }
    await _stopLocalCapture(clearSession: true);
    await _messageSub?.cancel();
    _messageSub = null;
    await _disconnectSub?.cancel();
    _disconnectSub = null;
    control.disconnect();
    _keys = null;
  }
}

/// 会话配置（写入 App Group / Service 共享，供采集组件读取）。
class SessionConfig {
  final String targetHost;
  final int audioPort;
  final int streamId;
  final Uint8List audioKey;
  final int sampleRate;
  final int channels;
  final int frameDurationMs;
  final int bitrate;

  SessionConfig({
    required this.targetHost,
    required this.audioPort,
    required this.streamId,
    required this.audioKey,
    required this.sampleRate,
    required this.channels,
    required this.frameDurationMs,
    required this.bitrate,
  });

  String toJson() => json.encode({
    'target_host': targetHost,
    'audio_port': audioPort,
    'stream_id': streamId,
    'audio_key': base64Encode(audioKey),
    'sample_rate': sampleRate,
    'channels': channels,
    'frame_duration_ms': frameDurationMs,
    'bitrate': bitrate,
  });
}
