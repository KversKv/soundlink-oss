// PairingStateReader.swift
//
// 从 App Group 共享容器读取由 Flutter 主 App 写入的会话配置。
// 配置 JSON schema 见 SessionConfig.toJson()（mobile/flutter_app/.../pairing_service.dart）。

import Foundation

struct SessionConfig: Codable {
    let targetHost: String
    let audioPort: Int
    let streamId: Int
    let audioKey: String
    let sampleRate: Int
    let channels: Int
    let frameDurationMs: Int
    let bitrate: Int

    enum CodingKeys: String, CodingKey {
        case targetHost = "target_host"
        case audioPort = "audio_port"
        case streamId = "stream_id"
        case audioKey = "audio_key"
        case sampleRate = "sample_rate"
        case channels
        case frameDurationMs = "frame_duration_ms"
        case bitrate
    }

    var runtimeBaseline: SessionConfig {
        SessionConfig(
            targetHost: targetHost,
            audioPort: audioPort,
            streamId: streamId,
            audioKey: audioKey,
            sampleRate: 48000,
            channels: 2,
            frameDurationMs: 10,
            bitrate: bitrate)
    }

    /// 阶段 P：会话格式白名单归一化（Mono/20ms 等；采样率受 Opus 限制固定 48kHz）。
    var sessionNormalized: SessionConfig {
        let sr = sampleRate == 48000 ? sampleRate : 48000
        let ch = [1, 2].contains(channels) ? channels : 2
        let fd = [10, 20].contains(frameDurationMs) ? frameDurationMs : 10
        return SessionConfig(
            targetHost: targetHost,
            audioPort: audioPort,
            streamId: streamId,
            audioKey: audioKey,
            sampleRate: sr,
            channels: ch,
            frameDurationMs: fd,
            bitrate: bitrate)
    }

    /// 解码 audio_key 为原始 32 字节。
    func audioKeyBytes() -> Data {
        Data(base64Encoded: audioKey) ?? Data()
    }
}

enum PairingStateReader {
    static let appGroupId = "group.com.soundlink"
    static let configKey = "soundlink.session.config"
    static let stopRequestedKey = "soundlink.stop_requested"
    /// Extension 停止原因日志键（主 App 读取用于排查"直播已停止"）。
    static let stopReasonKey = "soundlink.stop_reason"
    static let stopReasonTimestampKey = "soundlink.stop_reason_ts"
    /// N3：主 App 写入的目标码率（bps），Extension 每帧读取并热下发。
    static let pendingBitrateKey = "soundlink.pending_bitrate"

    /// 读取最新会话配置；不存在或解析失败返回 nil。
    static func read() -> SessionConfig? {
        guard let defaults = UserDefaults(suiteName: appGroupId) else { return nil }
        guard let json = defaults.string(forKey: configKey)?.data(using: .utf8) else {
            return nil
        }
        return try? JSONDecoder().decode(SessionConfig.self, from: json)
    }

    /// 主 App 调用：写入配置。
    static func write(_ config: SessionConfig) {
        guard let defaults = UserDefaults(suiteName: appGroupId) else { return }
        if let json = try? JSONEncoder().encode(config),
           let str = String(data: json, encoding: .utf8) {
            defaults.set(str, forKey: configKey)
            defaults.set(false, forKey: stopRequestedKey)
        }
    }

    static func requestStop() {
        UserDefaults(suiteName: appGroupId)?.set(true, forKey: stopRequestedKey)
    }

    /// Extension 停止时记录原因（主 App 读取用于排查）。
    static func recordStopReason(_ reason: String) {
        guard let defaults = UserDefaults(suiteName: appGroupId) else { return }
        defaults.set(reason, forKey: stopReasonKey)
        defaults.set(Date().timeIntervalSince1970, forKey: stopReasonTimestampKey)
    }

    /// 读取并清除停止原因（主 App 调用）。
    static func popStopReason() -> (reason: String, ts: TimeInterval)? {
        guard let defaults = UserDefaults(suiteName: appGroupId) else { return nil }
        guard let reason = defaults.string(forKey: stopReasonKey) else { return nil }
        let ts = defaults.double(forKey: stopReasonTimestampKey)
        defaults.removeObject(forKey: stopReasonKey)
        defaults.removeObject(forKey: stopReasonTimestampKey)
        return (reason, ts)
    }

    /// 主 App 调用：清除配置（停止后）。
    static func clear() {
        guard let defaults = UserDefaults(suiteName: appGroupId) else { return }
        defaults.removeObject(forKey: configKey)
        defaults.removeObject(forKey: stopRequestedKey)
    }
}
