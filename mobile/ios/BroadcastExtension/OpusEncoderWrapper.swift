// OpusEncoderWrapper.swift
//
// 封装 libopus 编码器。基线：48kHz / Stereo / 10ms 帧 / 128kbps。
// 输入 PCM Int16 交错（960 样本/帧 = 480 样本/声道 × 2），输出 Opus 字节。
//
// 依赖：libopus（xcframework / SwiftPM），经 Bridging Header 导入 <opus/opus.h>。
// ctl 请求码与 libopus 一致（见 opus_defines.h）。

import Foundation
import Opus

// libopus 请求码（opus_defines.h 节选）。
private let OPUS_SET_BITRATE_REQUEST: Int32 = 4002
private let OPUS_SET_COMPLEXITY_REQUEST: Int32 = 4010
private let OPUS_SET_SIGNAL_REQUEST: Int32 = 4024
private let OPUS_SET_PACKET_LOSS_PERC_REQUEST: Int32 = 4014
private let OPUS_APPLICATION_AUDIO: Int32 = 2049
private let OPUS_SIGNAL_MUSIC: Int32 = 3002

/// Opus 编码器封装（非线程安全；仅在 Extension 串行处理线程使用）。
final class OpusEncoderWrapper {
    private var state: OpaquePointer?
    private let sampleRate: Int32
    private let channels: Int32
    private let frameSize: Int32 // 每帧每声道样本数（480）

    /// 最大编码输出字节数（按 1276 上限）。
    private let maxDataBytes: Int32 = 1276

    init(sampleRate: Int = 48000, channels: Int = 2, bitrate: Int = 128000) {
        self.sampleRate = Int32(sampleRate)
        self.channels = Int32(channels)
        self.frameSize = Int32(sampleRate / 1000 * 10) // 10ms

        var err: Int32 = 0
        guard let st = opus_encoder_create(self.sampleRate, self.channels, OPUS_APPLICATION_AUDIO, &err),
              err == 0 else {
            return
        }
        self.state = st
        opus_encoder_ctl(st, OPUS_SET_BITRATE_REQUEST, Int32(bitrate))
        opus_encoder_ctl(st, OPUS_SET_COMPLEXITY_REQUEST, Int32(10))
        opus_encoder_ctl(st, OPUS_SET_SIGNAL_REQUEST, OPUS_SIGNAL_MUSIC)
        opus_encoder_ctl(st, OPUS_SET_PACKET_LOSS_PERC_REQUEST, Int32(0))
    }

    deinit {
        if let st = state { opus_encoder_destroy(st) }
    }

    /// 编码一帧 PCM（Int16 交错）。返回 Opus 字节，失败返回 nil。
    func encode(_ pcm: UnsafePointer<Int16>, frameSize: Int) -> Data? {
        guard let st = state else { return nil }
        var buf = [UInt8](repeating: 0, count: Int(maxDataBytes))
        let n = opus_encode(st, pcm, Int32(frameSize), &buf, maxDataBytes)
        guard n > 0 else { return nil }
        return Data(buf.prefix(Int(n)))
    }

    /// 便捷：从 Data(Int16 交错) 编码。
    func encode(_ pcmData: Data) -> Data? {
        guard pcmData.count >= MemoryLayout<Int16>.size * Int(channels) * Int(frameSize) else {
            return nil
        }
        return pcmData.withUnsafeBytes { (raw: UnsafeRawBufferPointer) -> Data? in
            guard let base = raw.baseAddress?.assumingMemoryBound(to: Int16.self) else { return nil }
            return encode(base, frameSize: Int(frameSize))
        }
    }

    var samplesPerFramePerChannel: Int { Int(frameSize) }
}
