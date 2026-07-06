// SoundLink 移动端入口。
//
// 文件内 DEBUG / DUMP_ENABLE 故意采用 UPPER_SNAKE_CASE，与桌面端 Rust、
// iOS Swift、Android Kotlin 的命名保持一致，便于跨端检索；此处整体豁免
// constant_identifier_names 规则。
// ignore_for_file: constant_identifier_names

import 'package:flutter/material.dart';

import 'app.dart';

/// 调试开关（开发期临时便利）。
///
/// 设为 `true` 后：
/// 1. 配对页输入框默认填充 `12345678`（与桌面端 DEBUG 模式生成的固定配对码一致）。
/// 2. 设备发现页“手动 IP”对话框默认填充 `10.31.30.41`，方便连调试机。
/// 3. 默认开启采集 PCM/Opus 转储（[DUMP_ENABLE] 跟随此值）。
///
/// 发布前务必改回 `false`。
const bool DEBUG = true;

/// 音频 RAW Data 转储开关。
///
/// `true` 时各原生采集端（iOS BroadcastExtension / Android Service）会把
/// 采集后 PCM、Opus 编码帧写到 app 私有目录 / 公共 Download 目录，
/// 便于用 Audacity / ffmpeg 分析各阶段数据。
///
/// 默认跟随 [DEBUG]；如需在非 DEBUG 模式下独立开启转储，改为显式 `true`。
const bool DUMP_ENABLE = DEBUG;

void main() {
  runApp(const SoundLinkApp());
}
