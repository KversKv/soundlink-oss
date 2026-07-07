// AudioPacket 二进制编解码（UDP 载荷）。
//
// 固定头部 32 字节（大端）+ ChaCha20-Poly1305 密文 + 16B 认证标签。
// 与 docs/First/11-implementation-spec.md §2 字节级对齐。
// 与桌面端 desktop/src-tauri/src/network/packet.rs 字节级互通。

import 'dart:typed_data';

import '../constants.dart' as k;
import '../crypto/chacha20_poly1305.dart';

/// AudioPacket 头部（32 字节，大端）。`payloadLen` 在 encode 时被覆写。
class AudioPacketHeader {
  final int streamId;
  final int sequence;
  final int timestamp; // u64 采样计数
  final int codec;
  final int channels;
  final int frameDurationMs;
  final int flags;
  final int sampleRate;
  int payloadLen; // encode 时覆写为明文长度

  AudioPacketHeader({
    required this.streamId,
    required this.sequence,
    required this.timestamp,
    required this.codec,
    required this.channels,
    required this.frameDurationMs,
    required this.flags,
    required this.sampleRate,
    this.payloadLen = 0,
  });

  /// 默认基线头部（48kHz/Stereo/Opus/10ms）。
  factory AudioPacketHeader.baseline({
    required int streamId,
    required int sequence,
    required int timestamp,
    int flags = 0,
  }) => AudioPacketHeader(
    streamId: streamId,
    sequence: sequence,
    timestamp: timestamp,
    codec: k.codecOpus,
    channels: k.channels,
    frameDurationMs: k.frameDurationMs,
    flags: flags,
    sampleRate: k.sampleRate,
  );

  /// 序列化为 32 字节大端头部。
  Uint8List toBytes() {
    final buf = ByteData(k.headerLen);
    buf.setUint16(0, k.magic, Endian.big);
    buf.setUint8(2, k.protocolVersion);
    buf.setUint8(3, k.headerLen);
    buf.setUint32(4, streamId, Endian.big);
    buf.setUint32(8, sequence, Endian.big);
    buf.setUint64(12, timestamp, Endian.big);
    buf.setUint8(20, codec);
    buf.setUint8(21, channels);
    buf.setUint8(22, frameDurationMs);
    buf.setUint8(23, flags);
    buf.setUint32(24, sampleRate, Endian.big);
    buf.setUint16(28, payloadLen, Endian.big);
    // buf[30..32] reserved = 0
    return buf.buffer.asUint8List();
  }

  /// 从 32 字节解析头部并校验 magic/version/header_len。
  static AudioPacketHeader fromBytes(Uint8List buf) {
    if (buf.length < k.headerLen) {
      throw AudioPacketException('包过短：${buf.length} 字节，需至少 ${k.headerLen}');
    }
    final bd = ByteData.sublistView(buf);
    final m = bd.getUint16(0, Endian.big);
    if (m != k.magic) {
      throw AudioPacketException('魔数错误：0x${m.toRadixString(16).toUpperCase()}');
    }
    final v = bd.getUint8(2);
    if (v != k.protocolVersion) {
      throw AudioPacketException('协议版本不兼容：$v');
    }
    final hl = bd.getUint8(3);
    if (hl != k.headerLen) {
      throw AudioPacketException('头部长度错误：$hl，应为 ${k.headerLen}');
    }
    return AudioPacketHeader(
      streamId: bd.getUint32(4, Endian.big),
      sequence: bd.getUint32(8, Endian.big),
      timestamp: bd.getUint64(12, Endian.big),
      codec: bd.getUint8(20),
      channels: bd.getUint8(21),
      frameDurationMs: bd.getUint8(22),
      flags: bd.getUint8(23),
      sampleRate: bd.getUint32(24, Endian.big),
      payloadLen: bd.getUint16(28, Endian.big),
    );
  }

  @override
  String toString() =>
      'AudioPacketHeader(stream=$streamId seq=$sequence ts=$timestamp codec=$codec ch=$channels dur=${frameDurationMs}ms sr=$sampleRate len=$payloadLen flags=$flags)';
}

class AudioPacketException implements Exception {
  final String message;
  AudioPacketException(this.message);
  @override
  String toString() => 'AudioPacketException: $message';
}

/// 构造 AEAD nonce：stream_id(4 BE) ‖ sequence(4 BE) ‖ 0x00000000(4)。
Uint8List buildNonce(int streamId, int sequence) {
  final nonce = Uint8List(k.aeadNonceLen);
  final bd = ByteData.sublistView(nonce);
  bd.setUint32(0, streamId, Endian.big);
  bd.setUint32(4, sequence, Endian.big);
  return nonce;
}

/// 编码 AudioPacket：header ‖ ciphertext ‖ tag。
///
/// - [audioKey]：会话音频密钥（32B）。
/// - [header]：头部（payloadLen 会被本函数覆写为 plaintext 长度）。
/// - [plaintext]：Opus 帧。
/// 返回 header ‖ (ciphertext‖tag)。
Future<Uint8List> encodePacket(
  Uint8List audioKey,
  AudioPacketHeader header,
  Uint8List plaintext,
) async {
  header.payloadLen = plaintext.length;
  final headerBytes = header.toBytes();
  final nonce = buildNonce(header.streamId, header.sequence);
  // ChaCha20-Poly1305 返回 ciphertext ‖ tag（尾部 16B），与 Rust crate 一致。
  final cipherWithTag = await chacha20Poly1305Encrypt(
    key: audioKey,
    nonce: nonce,
    plaintext: plaintext,
    aad: headerBytes,
  );
  final out = Uint8List(k.headerLen + cipherWithTag.length);
  out.setRange(0, k.headerLen, headerBytes);
  out.setRange(k.headerLen, out.length, cipherWithTag);
  return out;
}

/// 解码 AudioPacket：校验头部 → AEAD 解密 → 返回 Opus 帧明文。
Future<Uint8List> decodePacket(Uint8List audioKey, Uint8List buf) async {
  if (buf.length < k.headerLen) {
    throw AudioPacketException('包过短：${buf.length} 字节，需至少 ${k.headerLen}');
  }
  final header = AudioPacketHeader.fromBytes(buf);
  final declared = header.payloadLen;
  final cipherLen = buf.length - k.headerLen;
  if (declared + k.aeadTagLen != cipherLen) {
    throw AudioPacketException(
      'payload_len 与实际包长不符：声明 $declared，实际 $cipherLen',
    );
  }
  final nonce = buildNonce(header.streamId, header.sequence);
  final cipherWithTag = Uint8List.sublistView(buf, k.headerLen);
  try {
    return await chacha20Poly1305Decrypt(
      key: audioKey,
      nonce: nonce,
      ciphertext: cipherWithTag,
      aad: Uint8List.sublistView(buf, 0, k.headerLen),
    );
  } catch (_) {
    throw AudioPacketException('AEAD 解密/校验失败');
  }
}
