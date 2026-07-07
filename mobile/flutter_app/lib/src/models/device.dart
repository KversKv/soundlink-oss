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
