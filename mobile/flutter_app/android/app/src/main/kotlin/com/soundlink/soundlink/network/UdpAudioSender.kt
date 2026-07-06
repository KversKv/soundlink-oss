// UdpAudioSender.kt
//
// 构建 AudioPacket（32B 大端头 + ChaCha20-Poly1305 密文‖tag）并 UDP 发送。
// 与 desktop/src-tauri/src/network/packet.rs 字节级互通（spec §2）。
// 加密用 BouncyCastle ChaCha20-Poly1305（允许自定义 12B nonce）。

package com.soundlink.soundlink.network

import org.bouncycastle.crypto.engines.ChaCha20Poly1305
import org.bouncycastle.crypto.macs.AEADMac
import org.bouncycastle.crypto.params.AEADParameters
import org.bouncycastle.crypto.params.KeyParameter
import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.InetAddress
import java.nio.ByteBuffer
import java.nio.ByteOrder

data class SenderConfig(
    val targetHost: String,
    val audioPort: Int,
    val streamId: Int,
    val audioKey: ByteArray,      // 32B
    val sampleRate: Int,
    val channels: Int,
    val frameDurationMs: Int,
    val bitrate: Int,
)

class UdpAudioSender(private val cfg: SenderConfig) : AutoCloseable {

    private val key = KeyParameter(cfg.audioKey)
    private var sequence = 0
    private var timestamp = 0L
    private val socket = DatagramSocket()
    private val dest = InetAddress.getByName(cfg.targetHost)

    @Synchronized
    fun send(opusFrame: ByteArray, streamEnd: Boolean = false): Boolean {
        val header = buildHeader(opusFrame.size, if (streamEnd) FLAG_STREAM_END else 0)
        val cipher = encrypt(opusFrame, header)

        val packet = ByteArray(header.size + cipher.size)
        System.arraycopy(header, 0, packet, 0, header.size)
        System.arraycopy(cipher, 0, packet, header.size, cipher.size)

        return try {
            socket.send(DatagramPacket(packet, packet.size, dest, cfg.audioPort))
            sequence++
            timestamp += (cfg.sampleRate / 1000 * cfg.frameDurationMs).toLong() // +480
            true
        } catch (e: Exception) {
            false
        }
    }

    /** 构建 32 字节大端头部。 */
    private fun buildHeader(payloadLen: Int, flags: Int): ByteArray {
        val bb = ByteBuffer.allocate(HEADER_LEN).order(ByteOrder.BIG_ENDIAN)
        bb.putShort(MAGIC)              // 0..2 magic
        bb.put(VERSION.toByte())        // 2 version
        bb.put(HEADER_LEN.toByte())     // 3 header_len
        bb.putInt(cfg.streamId)         // 4..8 stream_id
        bb.putInt(sequence)             // 8..12 sequence
        bb.putLong(timestamp)           // 12..20 timestamp
        bb.put(CODEC_OPUS.toByte())     // 20 codec
        bb.put(cfg.channels.toByte())   // 21 channels
        bb.put(cfg.frameDurationMs.toByte()) // 22 frame_duration_ms
        bb.put(flags.toByte())          // 23 flags
        bb.putInt(cfg.sampleRate)       // 24..28 sample_rate
        bb.putShort(payloadLen.toShort()) // 28..30 payload_len
        bb.putShort(0)                  // 30..32 reserved
        return bb.array()
    }

    /** nonce = stream_id(4 BE) ‖ sequence(4 BE) ‖ 0(4)。 */
    private fun buildNonce(): ByteArray {
        val bb = ByteBuffer.allocate(12).order(ByteOrder.BIG_ENDIAN)
        bb.putInt(cfg.streamId)
        bb.putInt(sequence)
        bb.putInt(0)
        return bb.array()
    }

    /** ChaCha20-Poly1305 AEAD，返回 ciphertext ‖ tag(16B)。 */
    private fun encrypt(plaintext: ByteArray, aad: ByteArray): ByteArray {
        val engine = ChaCha20Poly1305()
        engine.init(true, AEADParameters(key, 128, buildNonce(), aad))
        val out = ByteArray(plaintext.size + 16)
        val len = engine.processBytes(plaintext, 0, plaintext.size, out, 0)
        engine.doFinal(out, len)
        return out
    }

    override fun close() {
        socket.close()
    }

    companion object {
        const val MAGIC: Short = 0x534C
        const val VERSION = 1
        const val HEADER_LEN = 32
        const val CODEC_OPUS = 1
        const val FLAG_STREAM_END = 0x01
    }
}
