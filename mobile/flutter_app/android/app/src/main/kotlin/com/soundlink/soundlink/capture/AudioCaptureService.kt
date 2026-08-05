// AudioCaptureService.kt
//
// 前台 Service：承载 MediaProjection + AudioPlaybackCapture。
// 流程：读取配置 → 用授权结果创建 MediaProjection → AudioRecord 采集 PCM(48kHz Stereo Int16)
//       → OpusEncoder 编码 → UdpAudioSender 加密并发送 UDP。
// 需前台通知（合规要求）；API 29+。
// 详见 docs/First/03-audio-pipeline.md、08-platform-notes.md §3、11-implementation-spec.md §8.3。

package com.soundlink.soundlink.capture

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.media.AudioAttributes
import android.media.AudioFormat
import android.media.AudioPlaybackCaptureConfiguration
import android.media.AudioRecord
import android.media.projection.MediaProjection
import android.media.projection.MediaProjectionManager
import android.os.Build
import android.os.IBinder
import android.provider.MediaStore
import android.util.Log
import com.soundlink.soundlink.codec.OpusEncoder
import com.soundlink.soundlink.network.SenderConfig
import com.soundlink.soundlink.network.UdpAudioSender
import com.soundlink.soundlink.network.normalizedSession
import org.json.JSONObject
import java.io.File
import java.io.FileOutputStream
import java.io.OutputStream

class AudioCaptureService : Service() {

    private var captureThread: Thread? = null
    private var mediaProjection: MediaProjection? = null
    private var audioRecord: AudioRecord? = null
    private var encoder: OpusEncoder? = null
    private var sender: UdpAudioSender? = null
    @Volatile private var running = false
    // N3：本次流起始码率（bps），captureLoop 以此跟踪热调整。
    private var startBitrate = 0
    // 阶段 P：归一化后的会话配置（采集循环按此做格式转换与凑帧）。
    private var sessionCfg: SenderConfig? = null
    // 会话格式样本累积缓冲（凑满一个会话帧编一次）。
    private var sessionAccum = ShortArray(0)

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_START -> startCapture(intent)
            ACTION_STOP -> stopCapture()
            ACTION_SET_BITRATE -> {
                // N3：Flutter 经 Plugin 写入 pending bitrate；captureLoop 每帧检测并热下发。
                val b = intent.getIntExtra(EXTRA_BITRATE, 0)
                if (b > 0) {
                    getSharedPreferences(PREFS, Context.MODE_PRIVATE)
                        .edit().putInt(KEY_PENDING_BITRATE, b).apply()
                }
            }
        }
        return START_NOT_STICKY
    }

    private fun startCapture(intent: Intent) {
        val resultCode = intent.getIntExtra(EXTRA_RESULT_CODE, 0)
        val data = intent.getParcelableExtra<Intent>(EXTRA_RESULT_DATA)
        if (data == null) {
            Log.e(TAG, "缺少 MediaProjection 授权数据")
            stopSelf()
            return
        }

        val cfg = readConfig() ?: run {
            Log.e(TAG, "缺少会话配置")
            stopSelf()
            return
        }

        startForegroundCompat()

        val mpm = getSystemService(Context.MEDIA_PROJECTION_SERVICE) as MediaProjectionManager
        mediaProjection = mpm.getMediaProjection(resultCode, data)
        if (mediaProjection == null) {
            Log.e(TAG, "MediaProjection 创建失败")
            stopSelf()
            return
        }

        try {
            // 阶段 P：按会话格式构造编码器/发送器（44.1k/Mono/20ms 等，经白名单归一化）。
            val runtimeCfg = cfg.normalizedSession()
            encoder = OpusEncoder(runtimeCfg.sampleRate, runtimeCfg.channels, runtimeCfg.bitrate)
            sender = UdpAudioSender(runtimeCfg)
            startBitrate = runtimeCfg.bitrate
            sessionCfg = runtimeCfg
        } catch (e: Exception) {
            Log.e(TAG, "编码器/发送器初始化失败", e)
            stopSelf()
            return
        }

        sessionAccum = ShortArray(0)
        running = true
        captureThread = Thread { captureLoop() }.apply { start() }
    }

    private fun captureLoop() {
        val projection = mediaProjection ?: return
        val sampleRate = 48000
        val channels = 2
        val frameSize = sampleRate / 1000 * 10 // 480 样本/声道

        // SDK 36 起旧构造函数 AudioPlaybackCaptureConfiguration(MediaProjection) 与
        // captureAudioOutput() 被隐藏，统一改用 Builder（API 29+ 公开 API）。
        // 必须显式 addMatchingUsage 至少一条，否则 AudioMixingRule 为空 build() 抛
        // IllegalArgumentException（"Cannot build AudioMixingRule with no rules"）。
        // 覆盖媒体/游戏/未知三类可捕获播放。
        val captureConfig = AudioPlaybackCaptureConfiguration.Builder(projection)
            .addMatchingUsage(AudioAttributes.USAGE_MEDIA)
            .addMatchingUsage(AudioAttributes.USAGE_GAME)
            .addMatchingUsage(AudioAttributes.USAGE_UNKNOWN)
            .build()

        val audioFormat = AudioFormat.Builder()
            .setEncoding(AudioFormat.ENCODING_PCM_16BIT)
            .setSampleRate(sampleRate)
            .setChannelMask(AudioFormat.CHANNEL_IN_STEREO)
            .build()

        val minBuf = AudioRecord.getMinBufferSize(sampleRate, AudioFormat.CHANNEL_IN_STEREO, AudioFormat.ENCODING_PCM_16BIT)
        val bufferSize = maxOf(minBuf * 2, frameSize * channels * 2 * 4)

        // SDK 36 起 AudioRecord.Builder 不再暴露 setPerformanceMode（PERFORMANCE_MODE_LOW_LATENCY
        // 常量亦被移除），用 bufferSize 与 AudioAttributes 控制延迟即可。
        val record = AudioRecord.Builder()
            .setAudioPlaybackCaptureConfig(captureConfig)
            .setAudioFormat(audioFormat)
            .setBufferSizeInBytes(bufferSize)
            .build()

        if (record.state != AudioRecord.STATE_INITIALIZED) {
            Log.e(TAG, "AudioRecord 初始化失败")
            running = false
            record.release()
            stopSelf()
            return
        }
        audioRecord = record
        record.startRecording()

        val pcm = ShortArray(frameSize * channels) // 960

        // 调试：是否保存采集到的原始 PCM（含 Opus 编码前后）。
        // 通过 SharedPreferences 的 dump_pcm=1 启用，文件写到公共 Download 目录。
        val sp = getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        val dumpPcm = sp.getBoolean("dump_pcm", false)
        // N3：跟踪当前生效码率，检测 pending 变化后热下发。
        var currentBitrate = startBitrate
        sp.edit().remove(KEY_PENDING_BITRATE).apply()
        var pcmDumpStream: OutputStream? = null
        var opusDumpStream: OutputStream? = null
        if (dumpPcm) {
            try {
                // 用 MediaStore 写入公共 Download 目录（Android 10+ 无需权限）。
                pcmDumpStream = openMediaStoreFile("capture_pcm.raw")
                opusDumpStream = openMediaStoreFile("capture_opus.bin")
                Log.i(TAG, "PCM 转储已启用（Download/soundlink_dump/）")
            } catch (e: Exception) {
                Log.w(TAG, "MediaStore 写入失败，回退 app 私有目录", e)
                try {
                    val dir = File(getExternalFilesDir(null), "soundlink_dump")
                    dir.mkdirs()
                    pcmDumpStream = FileOutputStream(File(dir, "capture_pcm.raw"))
                    opusDumpStream = FileOutputStream(File(dir, "capture_opus.bin"))
                    Log.i(TAG, "PCM 转储回退到私有目录：${dir.absolutePath}")
                } catch (e2: Exception) {
                    Log.w(TAG, "私有目录也失败", e2)
                }
            }
        }

        while (running) {
            // N3：检测码率热调整请求（SharedPreferences 轮询，每帧一次开销可忽略）。
            val pending = sp.getInt(KEY_PENDING_BITRATE, 0)
            if (pending > 0 && pending != currentBitrate) {
                encoder?.setBitrate(pending)
                Log.i(TAG, "编码码率热调整：${currentBitrate / 1000}kbps → ${pending / 1000}kbps")
                currentBitrate = pending
                sp.edit().remove(KEY_PENDING_BITRATE).apply()
            }

            val read = record.read(pcm, 0, pcm.size)
            if (read <= 0) continue
            if (read < pcm.size) continue // 不足一帧，丢弃

            // 转储原始 PCM（采集后、编码前）
            if (pcmDumpStream != null) {
                try {
                    val bb = java.nio.ByteBuffer.allocate(read * 2)
                    bb.order(java.nio.ByteOrder.LITTLE_ENDIAN)
                    for (i in 0 until read) bb.putShort(pcm[i])
                    pcmDumpStream.write(bb.array())
                } catch (e: Exception) {
                    Log.w(TAG, "PCM 转储写入失败", e)
                }
            }

            // 阶段 P：基线帧（48k/Stereo/10ms）→ 会话格式，累积凑满一个会话帧再编码。
            val cfg = sessionCfg
            if (cfg != null) {
                sessionAccum = SessionFormatConverter.toSession(pcm, cfg.sampleRate, cfg.channels)
                    .let { sessionAccum + it }
                val sessionFrameLen = cfg.sampleRate / 1000 * cfg.frameDurationMs * cfg.channels
                if (sessionAccum.size < sessionFrameLen) continue
                val sessionFrame = sessionAccum.copyOfRange(0, sessionFrameLen)
                sessionAccum = sessionAccum.copyOfRange(sessionFrameLen, sessionAccum.size)

                try {
                    val opus = encoder?.encode(sessionFrame) ?: continue

                    // 转储 Opus 帧（4 字节长度前缀 + 数据）
                    if (opusDumpStream != null) {
                        try {
                            val lb = java.nio.ByteBuffer.allocate(4)
                            lb.order(java.nio.ByteOrder.LITTLE_ENDIAN)
                            lb.putInt(opus.size)
                            opusDumpStream.write(lb.array())
                            opusDumpStream.write(opus)
                        } catch (e: Exception) {
                            Log.w(TAG, "Opus 转储写入失败", e)
                        }
                    }

                    sender?.send(opus)
                } catch (e: Exception) {
                    Log.w(TAG, "编码/发送异常", e)
                }
            }
        }

        pcmDumpStream?.close()
        opusDumpStream?.close()
        record.stop()
        record.release()
        if (audioRecord === record) {
            audioRecord = null
        }
    }

    /// 通过 MediaStore 在公共 Download/soundlink_dump/ 下创建文件，返回 OutputStream。
    /// Android 10+ 无需存储权限。已存在同名文件会覆盖（RELATIVE_PATH + DISPLAY_NAME）。
    private fun openMediaStoreFile(fileName: String): OutputStream {
        val resolver = contentResolver
        // 先删除旧文件（避免 MediaStore 自动改名）。
        val collection = MediaStore.Files.getContentUri("external")
        val sel = "${MediaStore.MediaColumns.RELATIVE_PATH} = ? AND ${MediaStore.MediaColumns.DISPLAY_NAME} = ?"
        val selArgs = arrayOf("${android.os.Environment.DIRECTORY_DOWNLOADS}/soundlink_dump/", fileName)
        resolver.delete(collection, sel, selArgs)
        // 插入新文件项。
        val values = android.content.ContentValues().apply {
            put(MediaStore.MediaColumns.DISPLAY_NAME, fileName)
            put(MediaStore.MediaColumns.RELATIVE_PATH, "${android.os.Environment.DIRECTORY_DOWNLOADS}/soundlink_dump")
            put(MediaStore.MediaColumns.MIME_TYPE, "application/octet-stream")
        }
        val uri = resolver.insert(collection, values)
            ?: throw java.io.IOException("MediaStore insert 失败：$fileName")
        return resolver.openOutputStream(uri, "w")
            ?: throw java.io.IOException("无法打开 OutputStream：$fileName")
    }

    private fun stopCapture() {
        running = false
        captureThread?.join(500)
        captureThread = null
        encoder?.close()
        encoder = null
        sender?.close()
        sender = null
        mediaProjection?.stop()
        mediaProjection = null
        stopForegroundCompat()
        stopSelf()
    }

    override fun onDestroy() {
        running = false
        audioRecord?.runCatching { stop() }
        audioRecord?.release()
        audioRecord = null
        encoder?.close()
        encoder = null
        sender?.close()
        sender = null
        mediaProjection?.stop()
        mediaProjection = null
        super.onDestroy()
    }

    // ===== 配置读取（由 SoundLinkPlugin 写入 SharedPreferences）=====
    private fun readConfig(): SenderConfig? {
        val sp = getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        val json = sp.getString(KEY_CONFIG, null) ?: return null
        return try {
            val o = JSONObject(json)
            SenderConfig(
                targetHost = o.getString("target_host"),
                audioPort = o.getInt("audio_port"),
                streamId = o.getInt("stream_id"),
                audioKey = android.util.Base64.decode(o.getString("audio_key"), android.util.Base64.DEFAULT),
                sampleRate = o.getInt("sample_rate"),
                channels = o.getInt("channels"),
                frameDurationMs = o.getInt("frame_duration_ms"),
                bitrate = o.getInt("bitrate"),
            )
        } catch (e: Exception) {
            Log.e(TAG, "配置解析失败", e); null
        }
    }

    // ===== 前台通知 =====
    private fun startForegroundCompat() {
        val nm = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(CHANNEL_ID, "SoundLink 采集", NotificationManager.IMPORTANCE_LOW)
            nm.createNotificationChannel(channel)
        }
        val notification: Notification = Notification.Builder(this, CHANNEL_ID)
            .setContentTitle("SoundLink 正在广播")
            .setContentText("音频正在流转到电脑")
            .setSmallIcon(android.R.drawable.ic_media_play)
            .setOngoing(true)
            .build()
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            startForeground(NOTIF_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PROJECTION)
        } else {
            startForeground(NOTIF_ID, notification)
        }
    }

    private fun stopForegroundCompat() {
        stopForeground(STOP_FOREGROUND_REMOVE)
    }

    companion object {
        private const val TAG = "AudioCaptureService"
        private const val CHANNEL_ID = "soundlink_capture"
        private const val NOTIF_ID = 0x51
        const val PREFS = "soundlink_session"
        const val KEY_CONFIG = "session_config"

        const val ACTION_START = "com.soundlink.START_CAPTURE"
        const val ACTION_STOP = "com.soundlink.STOP_CAPTURE"
        const val ACTION_SET_BITRATE = "com.soundlink.SET_BITRATE"
        const val EXTRA_RESULT_CODE = "result_code"
        const val EXTRA_RESULT_DATA = "result_data"
        const val EXTRA_BITRATE = "bitrate"
        const val KEY_PENDING_BITRATE = "pending_bitrate"
    }
}
