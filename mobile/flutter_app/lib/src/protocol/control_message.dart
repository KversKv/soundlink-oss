// 控制协议消息（JSON，换行分帧 / WebSocket text）。
//
// 与 docs/First/11-implementation-spec.md §3 字段级对齐。
// 与桌面端 control_server.rs 互通。所有消息含 type/msg_id/ts。

import 'dart:convert';

/// 控制消息公共字段。
abstract class ControlMessage {
  final String type;
  final String msgId;
  final int ts; // unix ms
  ControlMessage(this.type, this.msgId, this.ts);

  Map<String, dynamic> toJson();
  String toFrame() => '${json.encode(toJson())}\n';
}

/// hello (Sender→Receiver)
class HelloMsg extends ControlMessage {
  final int protocolVersion;
  final String deviceId;
  final String deviceName;
  final String role; // "sender"
  final String platform; // "ios" / "android"
  final Capabilities capabilities;

  HelloMsg({
    required String msgId,
    required int ts,
    required this.protocolVersion,
    required this.deviceId,
    required this.deviceName,
    required this.role,
    required this.platform,
    required this.capabilities,
  }) : super('hello', msgId, ts);

  @override
  Map<String, dynamic> toJson() => {
    'type': type,
    'msg_id': msgId,
    'ts': ts,
    'protocol_version': protocolVersion,
    'device_id': deviceId,
    'device_name': deviceName,
    'role': role,
    'platform': platform,
    'capabilities': capabilities.toJson(),
  };
}

class Capabilities {
  final List<String> codec; // ["opus"]
  final int sampleRate;
  final int channels;
  Capabilities({
    required this.codec,
    required this.sampleRate,
    required this.channels,
  });
  Map<String, dynamic> toJson() => {
    'codec': codec,
    'sample_rate': sampleRate,
    'channels': channels,
  };
}

/// hello_ack (Receiver→Sender)
class HelloAckMsg extends ControlMessage {
  final int protocolVersion;
  final String deviceId;
  final String deviceName;
  final bool pairingRequired;
  final bool trusted;

  HelloAckMsg({
    required String msgId,
    required int ts,
    required this.protocolVersion,
    required this.deviceId,
    required this.deviceName,
    required this.pairingRequired,
    required this.trusted,
  }) : super('hello_ack', msgId, ts);

  @override
  Map<String, dynamic> toJson() => {
    'type': type,
    'msg_id': msgId,
    'ts': ts,
    'protocol_version': protocolVersion,
    'device_id': deviceId,
    'device_name': deviceName,
    'pairing_required': pairingRequired,
    'trusted': trusted,
  };
}

/// pair_request (Sender→Receiver)
class PairRequestMsg extends ControlMessage {
  final String deviceId;
  final String senderPub; // base64 X25519 pub
  final String senderIdentityPub; // base64 Ed25519 pub
  final String proof; // base64 HMAC-SHA256

  PairRequestMsg({
    required String msgId,
    required int ts,
    required this.deviceId,
    required this.senderPub,
    required this.senderIdentityPub,
    required this.proof,
  }) : super('pair_request', msgId, ts);

  @override
  Map<String, dynamic> toJson() => {
    'type': type,
    'msg_id': msgId,
    'ts': ts,
    'device_id': deviceId,
    'sender_pub': senderPub,
    'sender_identity_pub': senderIdentityPub,
    'proof': proof,
  };
}

/// pair_response (Receiver→Sender)
class PairResponseMsg extends ControlMessage {
  final String result; // "ok" / "error"
  final String? receiverPub;
  final String? receiverIdentityPub;
  final String? proof;
  final ErrorDetail? error;

  PairResponseMsg({
    required String msgId,
    required int ts,
    required this.result,
    this.receiverPub,
    this.receiverIdentityPub,
    this.proof,
    this.error,
  }) : super('pair_response', msgId, ts);

  @override
  Map<String, dynamic> toJson() => {
    'type': type,
    'msg_id': msgId,
    'ts': ts,
    'result': result,
    if (receiverPub != null) 'receiver_pub': receiverPub,
    if (receiverIdentityPub != null)
      'receiver_identity_pub': receiverIdentityPub,
    if (proof != null) 'proof': proof,
    if (error != null) 'error': error!.toJson(),
  };
}

class ErrorDetail {
  final int code;
  final String message;
  ErrorDetail(this.code, this.message);
  Map<String, dynamic> toJson() => {'code': code, 'message': message};
}

/// stream_start (Sender→Receiver)
class StreamStartMsg extends ControlMessage {
  final int streamId;
  final int audioPort;
  final String codec; // "opus"
  final int sampleRate;
  final int channels;
  final int frameDurationMs;
  final int bitrate;

  StreamStartMsg({
    required String msgId,
    required int ts,
    required this.streamId,
    required this.audioPort,
    required this.codec,
    required this.sampleRate,
    required this.channels,
    required this.frameDurationMs,
    required this.bitrate,
  }) : super('stream_start', msgId, ts);

  @override
  Map<String, dynamic> toJson() => {
    'type': type,
    'msg_id': msgId,
    'ts': ts,
    'stream_id': streamId,
    'audio_port': audioPort,
    'codec': codec,
    'sample_rate': sampleRate,
    'channels': channels,
    'frame_duration_ms': frameDurationMs,
    'bitrate': bitrate,
  };
}

/// stream_start_ack (Receiver→Sender)
class StreamStartAckMsg extends ControlMessage {
  final int streamId;
  final String result;
  final int? receiverAudioPort;

  StreamStartAckMsg({
    required String msgId,
    required int ts,
    required this.streamId,
    required this.result,
    this.receiverAudioPort,
  }) : super('stream_start_ack', msgId, ts);

  @override
  Map<String, dynamic> toJson() => {
    'type': type,
    'msg_id': msgId,
    'ts': ts,
    'stream_id': streamId,
    'result': result,
    if (receiverAudioPort != null) 'receiver_audio_port': receiverAudioPort,
  };
}

/// stream_stop (Sender→Receiver)
class StreamStopMsg extends ControlMessage {
  final int streamId;
  StreamStopMsg({
    required String msgId,
    required int ts,
    required this.streamId,
  }) : super('stream_stop', msgId, ts);

  @override
  Map<String, dynamic> toJson() => {
    'type': type,
    'msg_id': msgId,
    'ts': ts,
    'stream_id': streamId,
  };
}

/// heartbeat (双向)
class HeartbeatMsg extends ControlMessage {
  HeartbeatMsg({required String msgId, required int ts})
    : super('heartbeat', msgId, ts);
  @override
  Map<String, dynamic> toJson() => {'type': type, 'msg_id': msgId, 'ts': ts};
}

/// stats (双向)
class StatsMsg extends ControlMessage {
  final int streamId;
  final int? packetsSent;
  final int? bitrate;
  final double? encodeMsAvg;
  final int? packetsRecv;
  final int? packetsLost;
  final int? jitterMs;
  final int? bufferMs;
  final int? estLatencyMs;

  StatsMsg({
    required String msgId,
    required int ts,
    required this.streamId,
    this.packetsSent,
    this.bitrate,
    this.encodeMsAvg,
    this.packetsRecv,
    this.packetsLost,
    this.jitterMs,
    this.bufferMs,
    this.estLatencyMs,
  }) : super('stats', msgId, ts);

  @override
  Map<String, dynamic> toJson() => {
    'type': type,
    'msg_id': msgId,
    'ts': ts,
    'stream_id': streamId,
    if (packetsSent != null) 'packets_sent': packetsSent,
    if (bitrate != null) 'bitrate': bitrate,
    if (encodeMsAvg != null) 'encode_ms_avg': encodeMsAvg,
    if (packetsRecv != null) 'packets_recv': packetsRecv,
    if (packetsLost != null) 'packets_lost': packetsLost,
    if (jitterMs != null) 'jitter_ms': jitterMs,
    if (bufferMs != null) 'buffer_ms': bufferMs,
    if (estLatencyMs != null) 'est_latency_ms': estLatencyMs,
  };
}

/// control_action (双向通用动作消息)
class ControlActionMsg extends ControlMessage {
  final String action;
  final Map<String, dynamic> payload;
  final String? target;
  final String? correlationId;

  ControlActionMsg({
    required String msgId,
    required int ts,
    required this.action,
    this.payload = const {},
    this.target,
    this.correlationId,
  }) : super('control_action', msgId, ts);

  @override
  Map<String, dynamic> toJson() => {
    'type': type,
    'msg_id': msgId,
    'ts': ts,
    'action': action,
    'payload': payload,
    if (target != null) 'target': target,
    if (correlationId != null) 'correlation_id': correlationId,
  };
}

/// control_action_ack (双向通用动作回执)
class ControlActionAckMsg extends ControlMessage {
  final String replyTo;
  final String action;
  final String result;
  final ErrorDetail? error;

  ControlActionAckMsg({
    required String msgId,
    required int ts,
    required this.replyTo,
    required this.action,
    required this.result,
    this.error,
  }) : super('control_action_ack', msgId, ts);

  @override
  Map<String, dynamic> toJson() => {
    'type': type,
    'msg_id': msgId,
    'ts': ts,
    'reply_to': replyTo,
    'action': action,
    'result': result,
    if (error != null) 'error': error!.toJson(),
  };
}

abstract final class ControlActions {
  static const mediaPlayPause = 'media.play_pause';
  static const mediaPrevious = 'media.previous';
  static const mediaNext = 'media.next';
  static const shortcutSet = 'shortcut.set';
  static const shortcutTrigger = 'shortcut.trigger';
}

/// error (双向)
class ErrorMsg extends ControlMessage {
  final ErrorDetail error;
  ErrorMsg({required String msgId, required int ts, required this.error})
    : super('error', msgId, ts);

  @override
  Map<String, dynamic> toJson() => {
    'type': type,
    'msg_id': msgId,
    'ts': ts,
    'error': error.toJson(),
  };
}

/// 解析一条 JSON 控制消息（去换行后）。
Map<String, dynamic> parseMessage(String line) =>
    json.decode(line) as Map<String, dynamic>;

/// 当前 unix 毫秒。
int nowMs() => DateTime.now().millisecondsSinceEpoch;

int _counter = 0;
String newMsgId(String prefix) {
  _counter++;
  return '$prefix-$_counter';
}
