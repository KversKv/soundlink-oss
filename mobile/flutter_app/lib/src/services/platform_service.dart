// 平台通道：Flutter 主 App ↔ 原生采集组件（iOS BroadcastExtension / Android Service）。
//
// 职责：
//   - writeSessionConfig：将配对派生的 audio_key / 目标地址写入 App Group（iOS）/
//     SharedPreferences（Android），供采集组件读取。
//   - startCapture / stopCapture：触发原生采集（iOS 跳转控制中心引导广播；
//     Android 启动前台 Service + MediaProjection）。
//   - getCaptureState：查询采集状态（EventChannel 流式上报）。
//
// 原生侧实现：
//   - iOS：mobile/flutter_app/ios/Runner/SoundLinkPlugin.swift（Runner target）
//   - Android：mobile/flutter_app/android/.../com/soundlink/soundlink/SoundLinkPlugin.kt
//
// 约束：音频数据不回传主 App；采集组件内直接编码发送。

import 'dart:async';

import 'package:flutter/services.dart';

import 'pairing_service.dart' show SessionConfig;

class PlatformService {
  static const MethodChannel _channel = MethodChannel('com.soundlink/platform');
  static const EventChannel _stateChannel = EventChannel(
    'com.soundlink/capture_state',
  );

  /// 写入会话配置到 App Group / EncryptedSharedPreferences。
  Future<void> writeSessionConfig(SessionConfig config) async {
    await _channel.invokeMethod('writeSessionConfig', {
      'config': config.toJson(),
    });
  }

  /// 开始采集（iOS：引导用户从控制中心开启广播；Android：启动前台 Service）。
  Future<void> startCapture() async {
    await _channel.invokeMethod('startCapture');
  }

  /// 停止采集。
  Future<void> stopCapture() async {
    await _channel.invokeMethod('stopCapture');
  }

  /// 请求 MediaProjection 授权（Android）。
  Future<bool> requestMediaProjection() async {
    final r = await _channel.invokeMethod('requestMediaProjection');
    return r == true;
  }

  /// 获取设备标识（用于 hello.device_id）。
  Future<String> getDeviceId() async {
    return await _channel.invokeMethod('getDeviceId') ?? 'unknown';
  }

  /// 调试：启用/禁用采集 PCM/Opus 转储（iOS 写 App Group；Android 写 Download，失败回退私有目录）。
  Future<void> setDumpPcm(bool enabled) async {
    await _channel.invokeMethod('setDumpPcm', {'enabled': enabled});
  }

  /// 采集状态事件流（state/packets_sent/bitrate 等）。
  Stream<Map<String, dynamic>> get captureState => _stateChannel
      .receiveBroadcastStream()
      .map((event) => Map<String, dynamic>.from(event as Map));
}

/// 采集状态（由原生上报）。
class CaptureState {
  final String state; // "idle" / "capturing" / "stopped" / "error"
  final int packetsSent;
  final int bitrate;
  final double encodeMsAvg;

  CaptureState({
    required this.state,
    this.packetsSent = 0,
    this.bitrate = 0,
    this.encodeMsAvg = 0,
  });

  factory CaptureState.fromMap(Map<String, dynamic> m) => CaptureState(
    state: m['state'] as String? ?? 'idle',
    packetsSent: m['packets_sent'] as int? ?? 0,
    bitrate: m['bitrate'] as int? ?? 0,
    encodeMsAvg: (m['encode_ms_avg'] as num?)?.toDouble() ?? 0,
  );
}
