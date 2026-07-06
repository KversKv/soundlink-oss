// 配对与密钥协商。第一版：X25519 + HKDF + HMAC（对齐 spec §5）。
//
// 与桌面端 desktop/src-tauri/src/pairing/key_exchange.rs 字节级一致：
//   - 配对码 → pairing_secret（HKDF）
//   - X25519 共享秘密 → session_master → audio_key / control_key
//   - HMAC 证明（Sender / Receiver 双向校验，防中间人）
//
// 升级 SPAKE2/SRP 见 docs/First/05-pairing-security.md。

import 'dart:convert';
import 'dart:typed_data';

import '../constants.dart';
import 'hkdf.dart';
import 'x25519.dart';

/// 配对码 → pairing_secret。
/// `pairing_secret = HKDF-SHA256(ikm=pairing_code, salt="soundlink-pair-v1", info=receiver_device_id, len=32)`
Future<Uint8List> derivePairingSecret(
  String pairingCode,
  String receiverDeviceId,
) async {
  return hkdfSha256(
    ikm: Uint8List.fromList(utf8.encode(pairingCode)),
    salt: Uint8List.fromList(pairingSalt),
    info: Uint8List.fromList(utf8.encode(receiverDeviceId)),
    length: aeadKeyLen,
  );
}

/// Sender 侧证明：
/// `proof = HMAC-SHA256(pairing_secret, sender_pub ‖ receiver_device_id ‖ protocol_version)`
Future<Uint8List> senderProof(
  Uint8List pairingSecret,
  Uint8List senderPub,
  String receiverDeviceId,
) async {
  final data = BytesBuilder()
    ..add(senderPub)
    ..add(utf8.encode(receiverDeviceId))
    ..addByte(protocolVersion);
  return hmacSha256(key: pairingSecret, data: data.toBytes());
}

/// Receiver 侧回证：
/// `proof' = HMAC-SHA256(pairing_secret, receiver_pub ‖ sender_pub ‖ receiver_device_id)`
Future<Uint8List> receiverProof(
  Uint8List pairingSecret,
  Uint8List receiverPub,
  Uint8List senderPub,
  String receiverDeviceId,
) async {
  final data = BytesBuilder()
    ..add(receiverPub)
    ..add(senderPub)
    ..add(utf8.encode(receiverDeviceId));
  return hmacSha256(key: pairingSecret, data: data.toBytes());
}

/// 校验 Sender 证明（恒定时间比较）。
Future<bool> verifySenderProof(
  Uint8List pairingSecret,
  Uint8List senderPub,
  String receiverDeviceId,
  Uint8List proof,
) async {
  final expected = await senderProof(pairingSecret, senderPub, receiverDeviceId);
  return constantTimeEq(expected, proof);
}

/// 校验 Receiver 回证。
Future<bool> verifyReceiverProof(
  Uint8List pairingSecret,
  Uint8List receiverPub,
  Uint8List senderPub,
  String receiverDeviceId,
  Uint8List proof,
) async {
  final expected =
      await receiverProof(pairingSecret, receiverPub, senderPub, receiverDeviceId);
  return constantTimeEq(expected, proof);
}

bool constantTimeEq(Uint8List a, Uint8List b) {
  if (a.length != b.length) return false;
  var r = 0;
  for (var i = 0; i < a.length; i++) {
    r |= a[i] ^ b[i];
  }
  return r == 0;
}

/// 会话密钥集合。
class SessionKeys {
  final Uint8List audioKey; // 32B
  final Uint8List controlKey; // 32B
  SessionKeys({required this.audioKey, required this.controlKey});
}

/// 由 X25519 共享秘密 + pairing_secret 派生会话密钥。
///
/// - `shared = X25519(own_priv, peer_pub)`
/// - `session_master = HKDF(ikm=shared, salt=pairing_secret, info="soundlink-session-v1", len=32)`
/// - `audio_key = HKDF(ikm=session_master, salt="", info="audio", len=32)`
/// - `control_key = HKDF(ikm=session_master, salt="", info="control", len=32)`
Future<SessionKeys> deriveSessionKeys(
  Uint8List sharedSecret,
  Uint8List pairingSecret,
) async {
  final sessionMaster = await hkdfSha256(
    ikm: sharedSecret,
    salt: pairingSecret,
    info: Uint8List.fromList(sessionInfo),
    length: aeadKeyLen,
  );
  final audioKey = await hkdfSha256(
    ikm: sessionMaster,
    salt: Uint8List(0),
    info: Uint8List.fromList(audioKeyInfo),
    length: aeadKeyLen,
  );
  final controlKey = await hkdfSha256(
    ikm: sessionMaster,
    salt: Uint8List(0),
    info: Uint8List.fromList(controlKeyInfo),
    length: aeadKeyLen,
  );
  return SessionKeys(audioKey: audioKey, controlKey: controlKey);
}

/// 一次性配对握手结果（Sender 视角）。
///
/// 调用方流程：
/// 1. [generateSenderKeyPair] 生成临时密钥对。
/// 2. 将 senderPub 发给 Receiver（pair_request）。
/// 3. 收到 receiverPub 与 receiver_proof，[verifyReceiverProof] 校验。
/// 4. [deriveSessionKeys] 得到 audio_key。
class SenderHandshake {
  final X25519KeyPair keyPair;
  final Uint8List pairingSecret;
  SenderHandshake(this.keyPair, this.pairingSecret);

  static Future<SenderHandshake> begin(
    String pairingCode,
    String receiverDeviceId,
  ) async {
    final kp = await X25519KeyPair.generate();
    final secret = await derivePairingSecret(pairingCode, receiverDeviceId);
    return SenderHandshake(kp, secret);
  }

  /// 计算 Sender 证明（随 pair_request 发送）。
  Future<Uint8List> computeProof(String receiverDeviceId) =>
      senderProof(pairingSecret, keyPair.publicKey, receiverDeviceId);

  /// 完成：用对端公钥派生会话密钥（应先校验 receiverProof）。
  Future<SessionKeys> complete(Uint8List receiverPub) async {
    final shared = await x25519SharedSecret(keyPair, receiverPub);
    return deriveSessionKeys(shared, pairingSecret);
  }
}
