// X25519 椭圆曲线密钥交换。
//
// 与桌面端 Rust `x25519-dalek` 字节级一致。
// 用于会话密钥协商：双方各生成临时密钥对，交换公钥，派生共享秘密。

import 'dart:typed_data';

import 'package:cryptography/cryptography.dart' as crypto;

/// X25519 临时密钥对。
class X25519KeyPair {
  final Uint8List publicKey; // 32B
  final crypto.SimpleKeyPair _keyPair;

  X25519KeyPair._(this.publicKey, this._keyPair);

  /// 生成新密钥对。
  static Future<X25519KeyPair> generate() async {
    final algo = crypto.X25519();
    final kp = await algo.newKeyPair();
    final pub = await kp.extractPublicKey();
    return X25519KeyPair._(Uint8List.fromList(pub.bytes), kp);
  }
}

/// 计算共享秘密：ownPriv × peerPub → 32B。
Future<Uint8List> x25519SharedSecret(
  X25519KeyPair own,
  Uint8List peerPublicKey,
) async {
  final algo = crypto.X25519();
  final remotePub = crypto.SimplePublicKey(
    peerPublicKey,
    type: crypto.KeyPairType.x25519,
  );
  final secret = await algo.sharedSecretKey(
    keyPair: own._keyPair,
    remotePublicKey: remotePub,
  );
  return Uint8List.fromList(await secret.extractBytes());
}
