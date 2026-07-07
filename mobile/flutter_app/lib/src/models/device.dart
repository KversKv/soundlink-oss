// 已发现的桌面设备。

import '../constants.dart' as k;

class DiscoveredDevice {
  final String deviceId;
  final String deviceName;
  final String role; // "receiver"
  final int protocolVersion;
  final bool pairingRequired;
  final String audioCodec;
  final int sampleRate;
  final int controlPort;
  final int audioPort;
  final String host; // IP 地址

  DiscoveredDevice({
    required this.deviceId,
    required this.deviceName,
    required this.role,
    required this.protocolVersion,
    required this.pairingRequired,
    required this.audioCodec,
    required this.sampleRate,
    required this.controlPort,
    required this.audioPort,
    required this.host,
  });

  Map<String, dynamic> toJson() => {
    'device_id': deviceId,
    'device_name': deviceName,
    'role': role,
    'protocol_version': protocolVersion,
    'pairing_required': pairingRequired,
    'audio_codec': audioCodec,
    'sample_rate': sampleRate,
    'control_port': controlPort,
    'audio_port': audioPort,
    'host': host,
  };

  factory DiscoveredDevice.fromJson(Map<String, dynamic> json) {
    return DiscoveredDevice(
      deviceId: json['device_id'] as String? ?? 'unknown',
      deviceName: json['device_name'] as String? ?? 'SoundLink Receiver',
      role: json['role'] as String? ?? 'receiver',
      protocolVersion: json['protocol_version'] as int? ?? k.protocolVersion,
      pairingRequired: json['pairing_required'] as bool? ?? true,
      audioCodec: json['audio_codec'] as String? ?? 'opus',
      sampleRate: json['sample_rate'] as int? ?? k.sampleRate,
      controlPort: json['control_port'] as int? ?? k.defaultControlPort,
      audioPort: json['audio_port'] as int? ?? k.defaultAudioPort,
      host: json['host'] as String? ?? '',
    );
  }

  /// 从 mDNS TXT 记录构造。
  factory DiscoveredDevice.fromTxt(String host, Map<String, String> txt) {
    return DiscoveredDevice(
      deviceId: txt['device_id'] ?? 'unknown',
      deviceName: txt['device_name'] ?? 'SoundLink Receiver',
      role: txt['role'] ?? 'receiver',
      protocolVersion:
          int.tryParse(txt['protocol_version'] ?? '1') ?? k.protocolVersion,
      pairingRequired:
          (txt['pairing_required'] ?? 'true').toLowerCase() == 'true',
      audioCodec: txt['audio_codec'] ?? 'opus',
      sampleRate:
          int.tryParse(txt['sample_rate'] ?? '${k.sampleRate}') ?? k.sampleRate,
      controlPort:
          int.tryParse(txt['control_port'] ?? '${k.defaultControlPort}') ??
          k.defaultControlPort,
      audioPort:
          int.tryParse(txt['audio_port'] ?? '${k.defaultAudioPort}') ??
          k.defaultAudioPort,
      host: host,
    );
  }

  @override
  String toString() =>
      'DiscoveredDevice($deviceName @ $host, ctrl=$controlPort audio=$audioPort, pair=$pairingRequired)';
}
