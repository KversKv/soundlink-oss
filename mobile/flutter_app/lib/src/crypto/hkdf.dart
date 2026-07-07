// HKDF-SHA256 与 HMAC-SHA256。
//
// 与桌面端 Rust `hkdf` / `hmac` crate 字节级一致（均遵循 RFC 5869 / RFC 2104）。
// 用于：配对码派生 pairing_secret、会话主密钥派生、audio_key/control_key 派生、
// 配对证明 HMAC。

import 'dart:typed_data';

import 'package:cryptography/cryptography.dart' as crypto;

/// HKDF-SHA256 派生密钥。
///
/// - [ikm]：输入密钥材料。
/// - [salt]：盐（可为空）。
/// - [info]：上下文信息。
/// - [length]：输出字节数。
Future<Uint8List> hkdfSha256({
  required Uint8List ikm,
  required Uint8List salt,
  required Uint8List info,
  required int length,
}) async {
  final hkdf = crypto.Hkdf(hmac: crypto.Hmac.sha256(), outputLength: length);
  final key = await hkdf.deriveKey(
    secretKey: crypto.SecretKey(ikm),
    nonce: salt,
    info: info,
  );
  return Uint8List.fromList(await key.extractBytes());
}

/// HMAC-SHA256。
Future<Uint8List> hmacSha256({
  required Uint8List key,
  required Uint8List data,
}) async {
  final hmac = crypto.Hmac.sha256();
  final mac = await hmac.calculateMac(data, secretKey: crypto.SecretKey(key));
  return Uint8List.fromList(mac.bytes);
}

/// SHA-256 摘要。
Future<Uint8List> sha256(Uint8List data) async {
  final h = crypto.Sha256();
  final hash = await h.hash(data);
  return Uint8List.fromList(hash.bytes);
}
