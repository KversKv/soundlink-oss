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

  PairingService({
    required this.control,
    required this.platform,
    required this.deviceId,
    required this.deviceName,
    required this.platformName,
    required this.identityPubB64,
  });

  SessionKeys? get sessionKeys => _keys;

  /// 执行完整握手与流启动。
  ///
  /// [pairingCode] 配对码（8 位数字）。已信任设备传 null（hello.trusted 路径）。
  /// [onState] 状态变更回调。
  Future<void> connectAndStart(
    DiscoveredDevice device, {
    String? pairingCode,
    required void Function(LinkState) onState,
  }) async {
    onState(LinkState.connecting);
    await control.connect();
    _disconnectSub?.cancel();
    _disconnectSub = control.onDisconnected.listen((_) async {
      await _stopLocalCapture();
      onState(LinkState.reconnecting);
    });
    await _messageSub?.cancel();
    _messageSub = control.messages.listen((msg) async {
      if (msg['type'] == 'stream_stop' || msg['type'] == 'error') {
        await _stopLocalCapture(clearSession: true);
        onState(LinkState.reconnecting);
      }
    });

    // 1) hello
    final hello = HelloMsg(
      msgId: newMsgId('c'),
      ts: nowMs(),
      protocolVersion: protocolVersion,
      deviceId: deviceId,
      deviceName: deviceName,
      role: 'sender',
      platform: platformName,
      capabilities: Capabilities(
        codec: ['opus'],
        sampleRate: sampleRate,
        channels: channels,
      ),
    );
    control.send(hello);

    final helloAck = await control.waitFor((m) => m['type'] == 'hello_ack');
    onState(LinkState.connected);

    final receiverDeviceId = helloAck['device_id'] as String;
    final pairingRequired = (helloAck['pairing_required'] as bool?) ?? true;
    final trusted = (helloAck['trusted'] as bool?) ?? false;

    // 2) 配对（如需且未信任）
    if (pairingRequired && !trusted) {
      if (pairingCode == null) {
        throw StateError('需要配对码但未提供');
      }
      onState(LinkState.pairing);
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
        onState(LinkState.error);
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
          onState(LinkState.error);
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
      onState(LinkState.paired);
    } else {
      // 已信任：跳过配对码，直接 X25519 协商（pairing_secret 用全 0 占位）。
      onState(LinkState.pairing);
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
      onState(LinkState.paired);
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
        sampleRate: sampleRate,
        channels: channels,
        frameDurationMs: frameDurationMs,
        bitrate: opusBitrate,
      ),
    );
    final ack = await control.waitFor((m) => m['type'] == 'stream_start_ack');
    if (ack['result'] != 'ok') {
      throw StateError('stream_start 被拒绝：${ack['error']}');
    }
    _audioPort = (ack['receiver_audio_port'] as int?) ?? _audioPort;

    // 4) 将会话配置写入 App Group / Service 共享，通知原生开始采集。
    final config = SessionConfig(
      targetHost: device.host,
      audioPort: _audioPort,
      streamId: _streamId,
      audioKey: _keys!.audioKey,
      sampleRate: sampleRate,
      channels: channels,
      frameDurationMs: frameDurationMs,
      bitrate: opusBitrate,
    );
    await platform.writeSessionConfig(config);
    await platform.startCapture();
    _startEventLoops();

    onState(LinkState.streaming);
  }

  void _startEventLoops() {
    _heartbeatTimer?.cancel();
    _statsTimer?.cancel();
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

  Future<void> _stopLocalCapture({bool clearSession = false}) async {
    _heartbeatTimer?.cancel();
    _statsTimer?.cancel();
    _heartbeatTimer = null;
    _statsTimer = null;
    await platform.stopCapture(clearSession: clearSession);
    _keys = null;
  }

  /// 停止流并断开。
  Future<void> stop() async {
    _heartbeatTimer?.cancel();
    _statsTimer?.cancel();
    _heartbeatTimer = null;
    _statsTimer = null;
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
