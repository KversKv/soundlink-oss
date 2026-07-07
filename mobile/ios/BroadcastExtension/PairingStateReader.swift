// PairingStateReader.swift
//
// 从 App Group 共享容器读取由 Flutter 主 App 写入的会话配置。
// 配置 JSON schema 见 SessionConfig.toJson()（mobile/flutter_app/.../pairing_service.dart）。

import Foundation

struct SessionConfig: Codable {
    let targetHost: String
    let audioPort: Int
    let streamId: Int
    let audioKey: String      // base64(32B)
    let sampleRate: Int
    let channels: Int
    let frameDurationMs: Int
    let bitrate: Int

    /// 解码 audio_key 为原始 32 字节。
    func audioKeyBytes() -> Data {
        Data(base64Encoded: audioKey) ?? Data()
    }
}

enum PairingStateReader {
    static let appGroupId = "group.com.soundlink"
    static let configKey = "soundlink.session.config"
    static let stopRequestedKey = "soundlink.stop_requested"

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

    /// 主 App 调用：清除配置（停止后）。
    static func clear() {
        guard let defaults = UserDefaults(suiteName: appGroupId) else { return }
        defaults.removeObject(forKey: configKey)
        defaults.removeObject(forKey: stopRequestedKey)
    }
}
