// ChaCha20-Poly1305 AEAD 封装。
//
// 与桌面端 Rust `chacha20poly1305` crate 字节级一致：
// 加密返回 ciphertext ‖ tag(16B) 拼接；解密接受同样拼接的输入。
// AAD = AudioPacket 头部 32 字节。

import 'dart:typed_data';

import 'package:cryptography/cryptography.dart' as crypto;

/// 加密：返回 ciphertext ‖ tag(16B)。
Future<Uint8List> chacha20Poly1305Encrypt({
  required Uint8List key, // 32B
  required Uint8List nonce, // 12B
  required Uint8List plaintext,
  required Uint8List aad,
}) async {
  final cipher = crypto.Chacha20.poly1305Aead();
  final box = await cipher.encrypt(
    plaintext,
    secretKey: crypto.SecretKey(key),
    nonce: nonce,
    aad: aad,
  );
  final ct = box.cipherText;
  final mac = box.mac.bytes;
  final out = Uint8List(ct.length + mac.length);
  out.setRange(0, ct.length, ct);
  out.setRange(ct.length, out.length, mac);
  return out;
}

/// 解密：输入为 ciphertext ‖ tag(16B)。
Future<Uint8List> chacha20Poly1305Decrypt({
  required Uint8List key,
  required Uint8List nonce,
  required Uint8List ciphertext,
  required Uint8List aad,
}) async {
  const tagLen = 16;
  if (ciphertext.length < tagLen) {
    throw ArgumentError('密文过短：${ciphertext.length}，需至少 $tagLen');
  }
  final ct = Uint8List.sublistView(ciphertext, 0, ciphertext.length - tagLen);
  final mac = crypto.Mac(
      Uint8List.sublistView(ciphertext, ciphertext.length - tagLen));
  final cipher = crypto.Chacha20.poly1305Aead();
  final box = crypto.SecretBox(ct, nonce: nonce, mac: mac);
  final plain = await cipher.decrypt(
    box,
    secretKey: crypto.SecretKey(key),
    aad: aad,
  );
  return Uint8List.fromList(plain);
}
