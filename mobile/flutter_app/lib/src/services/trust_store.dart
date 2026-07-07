// 信任存储（移动端）：持久化已配对桌面接收端的身份公钥与元数据。
//
// 第一版用 shared_preferences 存储（公钥非机密）。
// 后续升级 iOS Keychain / Android Keystore（见 05-pairing-security §5）。
//
// 与桌面端 trust_store.rs 对应：移动端信任的是「接收端」身份。

import 'dart:convert';
import 'dart:math';
import 'dart:typed_data';

import 'package:shared_preferences/shared_preferences.dart';

/// 已信任的桌面接收端。
class TrustedReceiver {
  final String deviceId;
  final String identityPubB64; // Ed25519 公钥（base64）
  final String deviceName;
  final String host; // 最后一次连接的 IP
  final int controlPort;
  final int audioPort;
  final int lastSeen; // unix 秒

  TrustedReceiver({
    required this.deviceId,
    required this.identityPubB64,
    required this.deviceName,
    required this.host,
    required this.controlPort,
    required this.audioPort,
    required this.lastSeen,
  });

  Map<String, dynamic> toJson() => {
    'device_id': deviceId,
    'identity_pub_b64': identityPubB64,
    'device_name': deviceName,
    'host': host,
    'control_port': controlPort,
    'audio_port': audioPort,
    'last_seen': lastSeen,
  };

  factory TrustedReceiver.fromJson(Map<String, dynamic> j) => TrustedReceiver(
    deviceId: j['device_id'] as String? ?? '',
    identityPubB64: j['identity_pub_b64'] as String? ?? '',
    deviceName: j['device_name'] as String? ?? 'SoundLink Receiver',
    host: j['host'] as String? ?? '',
    controlPort: j['control_port'] as int? ?? 47810,
    audioPort: j['audio_port'] as int? ?? 47811,
    lastSeen: j['last_seen'] as int? ?? 0,
  );
}

/// 信任存储管理器。
class TrustStore {
  static const _key = 'soundlink.trusted_receivers';

  /// 加载所有已信任接收端。
  static Future<List<TrustedReceiver>> loadAll() async {
    final prefs = await SharedPreferences.getInstance();
    final raw = prefs.getString(_key);
    if (raw == null || raw.isEmpty) return [];
    try {
      final list = json.decode(raw) as List;
      return list
          .map(
            (e) =>
                TrustedReceiver.fromJson(Map<String, dynamic>.from(e as Map)),
          )
          .toList();
    } catch (_) {
      return [];
    }
  }

  /// 查找已信任的接收端（按 device_id）。
  static Future<TrustedReceiver?> find(String deviceId) async {
    final all = await loadAll();
    for (final r in all) {
      if (r.deviceId == deviceId) return r;
    }
    return null;
  }

  /// 添加或更新信任。
  static Future<void> add(TrustedReceiver receiver) async {
    final all = await loadAll();
    all.removeWhere((r) => r.deviceId == receiver.deviceId);
    all.add(receiver);
    await _save(all);
  }

  /// 移除信任。
  static Future<bool> remove(String deviceId) async {
    final all = await loadAll();
    final before = all.length;
    all.removeWhere((r) => r.deviceId == deviceId);
    if (all.length < before) {
      await _save(all);
      return true;
    }
    return false;
  }

  /// 清空。
  static Future<void> clear() async {
    await _save([]);
  }

  static Future<void> _save(List<TrustedReceiver> all) async {
    final prefs = await SharedPreferences.getInstance();
    final raw = json.encode(all.map((r) => r.toJson()).toList());
    await prefs.setString(_key, raw);
  }
}

/// 移动端设备身份（持久化）。
///
/// 首次运行生成 32 字节随机身份公钥，后续复用。作为 pair_request.sender_identity_pub
/// 发送，桌面端保存后用于已信任设备的身份校验（跳过配对码）。
///
/// 第一版用随机字节作为身份标识（spec 05 §5：「第一版可用已存 identity_pub 简单校验，
/// 签名握手列为后续」）。后续可升级为 Ed25519 签名身份。
class MobileIdentity {
  static const _pubKeyKey = 'soundlink.identity_pub';
  static const _deviceIdKey = 'soundlink.device_id';

  /// 加载或生成设备身份。
  static Future<({String deviceId, String identityPubB64})>
  loadOrCreate() async {
    final prefs = await SharedPreferences.getInstance();

    // Device ID
    var deviceId = prefs.getString(_deviceIdKey) ?? '';
    if (deviceId.isEmpty) {
      deviceId =
          'mobile-${DateTime.now().millisecondsSinceEpoch.toRadixString(16)}';
      await prefs.setString(_deviceIdKey, deviceId);
    }

    // 身份公钥（32B 随机，作为稳定标识）
    var pubB64 = prefs.getString(_pubKeyKey) ?? '';
    if (pubB64.isEmpty) {
      final rngBytes = _randomBytes(32);
      pubB64 = base64Encode(rngBytes);
      await prefs.setString(_pubKeyKey, pubB64);
    }

    return (deviceId: deviceId, identityPubB64: pubB64);
  }
}

/// 生成密码学安全的随机字节。
Uint8List _randomBytes(int length) {
  final random = Random.secure();
  return Uint8List.fromList(
    List<int>.generate(length, (_) => random.nextInt(256)),
  );
}
