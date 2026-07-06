// AudioPacket + AEAD 往返测试：验证 Dart 实现与 spec §2 字节布局一致。
// 与 desktop/src-tauri/src/network/packet.rs 测试对齐。

import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:soundlink/src/constants.dart';
import 'package:soundlink/src/crypto/chacha20_poly1305.dart';
import 'package:soundlink/src/protocol/audio_packet.dart';

void main() {
  group('AudioPacketHeader', () {
    test('头部 32 字节往返', () {
      final h = AudioPacketHeader.baseline(streamId: 7, sequence: 42, timestamp: 480 * 42);
      final bytes = h.toBytes();
      expect(bytes.length, headerLen);

      final h2 = AudioPacketHeader.fromBytes(bytes);
      expect(h2.streamId, 7);
      expect(h2.sequence, 42);
      expect(h2.timestamp, 480 * 42);
      expect(h2.codec, codecOpus);
      expect(h2.channels, channels);
      expect(h2.sampleRate, sampleRate);
      expect(h2.frameDurationMs, frameDurationMs);
    });

    test('魔数错误被拒', () {
      final bytes = AudioPacketHeader.baseline(streamId: 1, sequence: 1, timestamp: 0).toBytes();
      bytes[0] = 0x00; // 破坏 magic
      expect(() => AudioPacketHeader.fromBytes(bytes),
          throwsA(isA<AudioPacketException>()));
    });

    test('版本不兼容被拒', () {
      final bytes = AudioPacketHeader.baseline(streamId: 1, sequence: 1, timestamp: 0).toBytes();
      bytes[2] = 99; // 破坏 version
      expect(() => AudioPacketHeader.fromBytes(bytes),
          throwsA(isA<AudioPacketException>()));
    });
  });

  group('AEAD packet roundtrip', () {
    test('编码→解码还原 Opus 帧', () async {
      final key = Uint8List.fromList(List.filled(aeadKeyLen, 0x42));
      final header = AudioPacketHeader.baseline(streamId: 1, sequence: 5, timestamp: 2400);
      final opusFrame = Uint8List.fromList(List.filled(80, 0xAB));

      final packet = await encodePacket(key, header, opusFrame);
      // 32 header + 80 cipher + 16 tag = 128
      expect(packet.length, headerLen + 80 + aeadTagLen);

      final plain = await decodePacket(key, packet);
      expect(plain, opusFrame);
    });

    test('错误密钥解密失败', () async {
      final key = Uint8List.fromList(List.filled(aeadKeyLen, 0x42));
      final header = AudioPacketHeader.baseline(streamId: 1, sequence: 5, timestamp: 2400);
      final packet = await encodePacket(key, header, Uint8List.fromList([1, 2, 3, 4]));

      final badKey = Uint8List.fromList(List.filled(aeadKeyLen, 0x99));
      expect(() => decodePacket(badKey, packet),
          throwsA(isA<AudioPacketException>()));
    });

    test('nonce 构造：stream_id‖sequence‖0', () {
      final nonce = buildNonce(0x11223344, 0xAABBCCDD);
      expect(nonce.length, aeadNonceLen);
      expect(nonce[0], 0x11);
      expect(nonce[1], 0x22);
      expect(nonce[2], 0x33);
      expect(nonce[3], 0x44);
      expect(nonce[4], 0xAA);
      expect(nonce[5], 0xBB);
      expect(nonce[6], 0xCC);
      expect(nonce[7], 0xDD);
      expect(nonce[8], 0);
      expect(nonce[11], 0);
    });
  });

  group('ChaCha20-Poly1305 直接往返', () {
    test('加密/解密一致', () async {
      final key = Uint8List.fromList(List.filled(32, 7));
      final nonce = Uint8List(12);
      final aad = Uint8List.fromList([1, 2, 3]);
      final plain = Uint8List.fromList(List.generate(50, (i) => i));
      final ct = await chacha20Poly1305Encrypt(
          key: key, nonce: nonce, plaintext: plain, aad: aad);
      expect(ct.length, plain.length + aeadTagLen);
      final back = await chacha20Poly1305Decrypt(
          key: key, nonce: nonce, ciphertext: ct, aad: aad);
      expect(back, plain);
    });
  });
}
