// UdpAudioSender.swift
//
// 构建 AudioPacket（32B 大端头 + ChaCha20-Poly1305 密文‖tag）并 UDP 发送。
// 与 desktop/src-tauri/src/network/packet.rs 字节级互通（spec §2）。
// 加密用 CryptoKit ChaChaPoly；UDP 用 BSD socket（轻量，适合 Extension）。

import Foundation
import CryptoKit
import Darwin

final class UdpAudioSender {
    private let config: SessionConfig
    private let key: SymmetricKey
    private var sequence: UInt32 = 0
    private var timestamp: UInt64 = 0
    private var sock: Int32 = -1
    private var dest: sockaddr_in = sockaddr_in()

    /// 常量（对齐 constants）。
    private let magic: UInt16 = 0x534C
    private let version: UInt8 = 1
    private let headerLen: UInt8 = 32
    private let codec: UInt8 = 1
    private let flagStreamEnd: UInt8 = 0x01

    init?(config: SessionConfig) {
        self.config = config
        let keyBytes = config.audioKeyBytes()
        guard keyBytes.count == 32 else { return nil }
        self.key = SymmetricKey(data: keyBytes)

        sock = socket(AF_INET, SOCK_DGRAM, 0)
        guard sock >= 0 else { return nil }
        dest.sin_family = sa_family_t(AF_INET)
        dest.sin_port = in_port_t(config.audioPort).bigEndian
        dest.sin_addr.s_addr = inet_addr(config.targetHost)
        if dest.sin_addr.s_addr == INADDR_NONE { return nil }
    }

    deinit {
        if sock >= 0 { close(sock) }
    }

    /// 加密并发送一帧 Opus 数据。返回是否成功。
    @discardableResult
    func send(opusFrame: Data, streamEnd: Bool = false) -> Bool {
        guard sock >= 0 else { return false }
        let header = buildHeader(
            payloadLen: UInt16(opusFrame.count),
            flags: streamEnd ? flagStreamEnd : 0)

        // nonce = stream_id(4 BE) ‖ sequence(4 BE) ‖ 0(4)
        let nonceBytes = buildNonce()
        guard let nonce = try? ChaChaPoly.Nonce(data: nonceBytes) else { return false }
        guard let sealed = try? ChaChaPoly.seal(
            opusFrame,
            using: key,
            nonce: nonce,
            authenticating: header) else {
            return false
        }

        // packet = header ‖ ciphertext ‖ tag
        var packet = header
        packet.append(sealed.ciphertext)
        packet.append(sealed.tag)

        let ok = packet.withUnsafeBytes { (raw: UnsafeRawBufferPointer) -> Bool in
            guard let base = raw.baseAddress else { return false }
            let sent = withUnsafePointer(to: &dest) { dPtr -> Int in
                dPtr.withMemoryRebound(to: sockaddr.self, capacity: 1) { sa in
                    sendto(sock, base, packet.count, 0, sa, socklen_t(MemoryLayout<sockaddr_in>.size))
                }
            }
            return sent == packet.count
        }

        sequence &+= 1
        timestamp &+= UInt64(config.sampleRate / 1000 * config.frameDurationMs) // +480
        return ok
    }

    /// 构建 32 字节大端头部。
    private func buildHeader(payloadLen: UInt16, flags: UInt8) -> Data {
        var d = Data(count: Int(headerLen))
        d[0] = UInt8(magic >> 8)
        d[1] = UInt8(magic & 0xFF)
        d[2] = version
        d[3] = headerLen
        writeBE32(&d, at: 4, UInt32(config.streamId))
        writeBE32(&d, at: 8, sequence)
        writeBE64(&d, at: 12, timestamp)
        d[20] = codec
        d[21] = UInt8(config.channels)
        d[22] = UInt8(config.frameDurationMs)
        d[23] = flags
        writeBE32(&d, at: 24, UInt32(config.sampleRate))
        d[28] = UInt8(payloadLen >> 8)
        d[29] = UInt8(payloadLen & 0xFF)
        d[30] = 0; d[31] = 0
        return d
    }

    private func buildNonce() -> Data {
        var n = Data(count: 12)
        writeBE32(&n, at: 0, UInt32(config.streamId))
        writeBE32(&n, at: 4, sequence)
        // 8..12 = 0
        return n
    }

    private func writeBE32(_ d: inout Data, at offset: Int, _ v: UInt32) {
        d[offset] = UInt8((v >> 24) & 0xFF)
        d[offset + 1] = UInt8((v >> 16) & 0xFF)
        d[offset + 2] = UInt8((v >> 8) & 0xFF)
        d[offset + 3] = UInt8(v & 0xFF)
    }

    private func writeBE64(_ d: inout Data, at offset: Int, _ v: UInt64) {
        for i in 0..<8 {
            d[offset + i] = UInt8((v >> ((7 - i) * 8)) & 0xFF)
        }
    }
}
