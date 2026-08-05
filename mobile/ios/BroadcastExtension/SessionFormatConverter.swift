// SessionFormatConverter.swift
//
// 会话格式转换（阶段 P · 参数动态化）。与 desktop format_convert.rs / Android SessionFormatConverter.kt 逻辑一致。
//
// AudioProcessor 始终输出基线 48kHz/Stereo/10ms 帧；编码前转换为会话格式（44.1k/Mono/20ms 等）：
// 声道映射（Stereo→Mono 平均 / Mono→Stereo 复制）+ 线性插值重采样。基线↔基线时直通。

import Foundation

enum SessionFormatConverter {
    static let baselineRate = 48000
    static let baselineChannels = 2

    /// 基线交错 PCM（Int16）→ 会话格式交错 PCM。
    static func toSession(_ input: [Int16], sessionRate: Int, sessionChannels: Int) -> [Int16] {
        if sessionRate == baselineRate && sessionChannels == baselineChannels {
            return input
        }
        let mapped = mapChannels(input, from: baselineChannels, to: sessionChannels)
        return resampleLinear(mapped, fromRate: baselineRate, toRate: sessionRate, channels: sessionChannels)
    }

    private static func mapChannels(_ input: [Int16], from fromCh: Int, to toCh: Int) -> [Int16] {
        if fromCh == toCh { return input }
        let frames = input.count / fromCh
        if fromCh == 2 && toCh == 1 {
            // Stereo → Mono：平均。
            var out = [Int16](repeating: 0, count: frames)
            for f in 0..<frames {
                let l = Int32(input[f * 2])
                let r = Int32(input[f * 2 + 1])
                out[f] = Int16((l + r) / 2)
            }
            return out
        }
        if fromCh == 1 && toCh == 2 {
            // Mono → Stereo：复制。
            var out = [Int16](repeating: 0, count: frames * 2)
            for f in 0..<frames {
                out[f * 2] = input[f]
                out[f * 2 + 1] = input[f]
            }
            return out
        }
        return input
    }

    private static func resampleLinear(_ input: [Int16], fromRate: Int, toRate: Int, channels: Int) -> [Int16] {
        if fromRate == toRate { return input }
        let inFrames = input.count / channels
        if inFrames == 0 { return [] }
        let ratio = Double(toRate) / Double(fromRate)
        let outFrames = Int((Double(inFrames) * ratio).rounded())
        var out = [Int16](repeating: 0, count: outFrames * channels)
        for of in 0..<outFrames {
            let srcPos = Double(of) / ratio
            let i0 = Int(srcPos.rounded(.down))
            let i1 = min(i0 + 1, inFrames - 1)
            let frac = Float(srcPos - Double(i0))
            for c in 0..<channels {
                let a = Float(input[i0 * channels + c])
                let b = Float(input[i1 * channels + c])
                out[of * channels + c] = Int16((a + (b - a) * frac).rounded())
            }
        }
        return out
    }
}
