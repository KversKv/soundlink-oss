// OpusEncoder.kt
//
// libopus 编码器 JNI 封装。基线：48kHz / Stereo / 10ms 帧 / 128kbps。
// 输入 PCM Int16 交错（960 样本/帧），输出 Opus 字节。
//
// 依赖：native lib libsoundlink_opus.so（见 src/main/cpp/opus_jni.c + CMakeLists.txt）。
// 由 CMake 在构建时编译 libopus 并链接；需 Android NDK。

package com.soundlink.soundlink.codec

class OpusEncoder(
    sampleRate: Int = 48000,
    channels: Int = 2,
    bitrate: Int = 128000,
) : AutoCloseable {

    private val channels: Int = channels
    private val frameSize: Int = sampleRate / 1000 * 10 // 480

    private val ptr: Long = nativeCreate(sampleRate, channels, bitrate)

    init {
        require(ptr != 0L) { "Opus 编码器初始化失败" }
    }

    /** 编码一帧 PCM Int16 交错，返回 Opus 字节；失败返回空。 */
    fun encode(pcm: ShortArray): ByteArray {
        require(pcm.size >= frameSize * channels) { "PCM 帧过短：${pcm.size}" }
        return nativeEncode(ptr, pcm, frameSize)
    }

    val samplesPerFramePerChannel: Int get() = frameSize

    override fun close() {
        if (ptr != 0L) nativeDestroy(ptr)
    }

    private companion object {
        init {
            System.loadLibrary("soundlink_opus")
        }
        @JvmStatic external fun nativeCreate(sampleRate: Int, channels: Int, bitrate: Int): Long
        @JvmStatic external fun nativeEncode(ptr: Long, pcm: ShortArray, frameSize: Int): ByteArray
        @JvmStatic external fun nativeDestroy(ptr: Long)
    }
}
