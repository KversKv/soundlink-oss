// 全局常量（单源）。对齐 shared/constants/README.md 与
// docs/First/11-implementation-spec.md §1。
// 修改这里须同步 desktop/src-tauri/src/constants.rs 与各端常量定义。

import 'dart:convert';

/// mDNS 服务类型。
const String mdnsServiceType = '_soundlink._udp.local.';

/// 协议版本。
const int protocolVersion = 1;

/// AudioPacket 魔数 "SL"（大端）。
const int magic = 0x534C;

/// AudioPacket 固定头部长度（字节）。
const int headerLen = 32;

/// 默认控制通道端口（TCP/WS）。
const int defaultControlPort = 47810;

/// 默认音频通道端口（UDP）。
const int defaultAudioPort = 47811;

/// 采样率。
const int sampleRate = 48000;

/// 声道数。
const int channels = 2;

/// Opus 帧长（毫秒）。
const int frameDurationMs = 10;

/// 每帧每声道样本数 = 48000 * 10 / 1000。
const int samplesPerFramePerChannel = 480;

/// Opus 起始码率。
const int opusBitrate = 128000;

const List<int> audioSampleRateOptions = [48000];
const List<int> audioChannelOptions = [2];
const List<int> audioFrameDurationOptions = [10];
const List<int> audioBitrateOptions = [64000, 96000, 128000, 160000, 192000];

/// 编码类型：Opus。
const int codecOpus = 1;

/// flags bit0：流末包。
const int flagStreamEnd = 0x01;

/// 默认 Jitter 缓冲（毫秒）。
const int defaultJitterMs = 80;

/// Jitter 三档（毫秒）。
const int jitterLowMs = 40;
const int jitterBalancedMs = 80;
const int jitterStableMs = 150;

/// 配对码位数。
const int pairingCodeDigits = 8;

/// 配对码有效期（秒）。
const int pairingCodeTtlSecs = 120;

/// 配对码最大尝试次数。
const int pairingCodeMaxAttempts = 5;

/// AEAD 密钥长度（字节）。
const int aeadKeyLen = 32;

/// AEAD nonce 长度（字节）。
const int aeadNonceLen = 12;

/// AEAD 认证标签长度（字节）。
const int aeadTagLen = 16;

/// 心跳间隔（秒）。
const int heartbeatIntervalSecs = 2;

/// 心跳超时（秒）。
const int heartbeatTimeoutSecs = 6;

/// HKDF 派生 salt。
final List<int> pairingSalt = utf8.encode('soundlink-pair-v1');

/// 会话密钥 HKDF info。
final List<int> sessionInfo = utf8.encode('soundlink-session-v1');

/// 音频密钥 HKDF info。
final List<int> audioKeyInfo = utf8.encode('audio');

/// 控制密钥 HKDF info。
final List<int> controlKeyInfo = utf8.encode('control');

/// 默认 stream_id。
const int defaultStreamId = 1;

/// 单帧 PCM 样本总数（交错，Int16）= 480 * 2。
const int frameSamplesTotal = samplesPerFramePerChannel * channels;

/// App Group 标识（iOS 主 App 与 BroadcastExtension 共享）。
const String appGroupId = 'group.com.soundlink';

/// App Group 共享键名：采集/会话配置（JSON）。
const String appGroupConfigKey = 'soundlink.session.config';
