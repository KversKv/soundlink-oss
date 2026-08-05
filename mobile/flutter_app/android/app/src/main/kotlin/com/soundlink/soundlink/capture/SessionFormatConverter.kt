// SessionFormatConverter.kt
//
// 会话格式转换（阶段 P · 参数动态化）。与 desktop/src-tauri/src/audio/format_convert.rs 逻辑一致。
//
// 采集始终基线 48kHz/Stereo/10ms；编码前转换为会话格式（44.1k/Mono/20ms 等）：
// 声道映射（Stereo→Mono 平均 / Mono→Stereo 复制）+ 线性插值重采样。
// 基线↔基线时直通，不引入额外延迟。

package com.soundlink.soundlink.capture

import kotlin.math.floor
import kotlin.math.roundToInt

/** 设备基线（采集侧固定）。 */
object Baseline {
    const val SAMPLE_RATE = 48000
    const val CHANNELS = 2
    const val FRAME_MS = 10
}

object SessionFormatConverter {

    /** 基线交错 PCM → 会话格式交错 PCM。 */
    fun toSession(
        input: ShortArray,
        sessionRate: Int,
        sessionChannels: Int,
    ): ShortArray {
        if (sessionRate == Baseline.SAMPLE_RATE && sessionChannels == Baseline.CHANNELS) {
            return input
        }
        val mapped = mapChannels(input, Baseline.CHANNELS, sessionChannels)
        return resampleLinear(mapped, Baseline.SAMPLE_RATE, sessionRate, sessionChannels)
    }

    /** 声道映射。每帧 = channels 个样本。 */
    private fun mapChannels(input: ShortArray, fromCh: Int, toCh: Int): ShortArray {
        if (fromCh == toCh) return input
        val frames = input.size / fromCh
        return when {
            fromCh == 2 && toCh == 1 -> {
                // Stereo → Mono：平均。
                val out = ShortArray(frames)
                for (f in 0 until frames) {
                    val l = input[f * 2].toInt()
                    val r = input[f * 2 + 1].toInt()
                    out[f] = ((l + r) / 2).toShort()
                }
                out
            }
            fromCh == 1 && toCh == 2 -> {
                // Mono → Stereo：复制。
                val out = ShortArray(frames * 2)
                for (f in 0 until frames) {
                    out[f * 2] = input[f]
                    out[f * 2 + 1] = input[f]
                }
                out
            }
            else -> input
        }
    }

    /** 线性插值重采样。 */
    private fun resampleLinear(
        input: ShortArray,
        fromRate: Int,
        toRate: Int,
        channels: Int,
    ): ShortArray {
        if (fromRate == toRate) return input
        val inFrames = input.size / channels
        if (inFrames == 0) return ShortArray(0)
        val ratio = toRate.toDouble() / fromRate.toDouble()
        val outFrames = (inFrames * ratio).roundToInt()
        val out = ShortArray(outFrames * channels)
        for (of in 0 until outFrames) {
            val srcPos = of / ratio
            val i0 = floor(srcPos).toInt()
            val i1 = (i0 + 1).coerceAtMost(inFrames - 1)
            val frac = (srcPos - i0).toFloat()
            for (c in 0 until channels) {
                val a = input[i0 * channels + c].toFloat()
                val b = input[i1 * channels + c].toFloat()
                out[of * channels + c] = (a + (b - a) * frac).toInt().toShort()
            }
        }
        return out
    }
}
