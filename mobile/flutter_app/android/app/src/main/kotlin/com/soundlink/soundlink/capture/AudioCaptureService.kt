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
import android.util.Log
import com.soundlink.soundlink.codec.OpusEncoder
import com.soundlink.soundlink.network.SenderConfig
import com.soundlink.soundlink.network.UdpAudioSender
import org.json.JSONObject

class AudioCaptureService : Service() {

    private var captureThread: Thread? = null
    private var mediaProjection: MediaProjection? = null
    private var audioRecord: AudioRecord? = null
    private var encoder: OpusEncoder? = null
    private var sender: UdpAudioSender? = null
    @Volatile private var running = false

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_START -> startCapture(intent)
            ACTION_STOP -> stopCapture()
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
            encoder = OpusEncoder(cfg.sampleRate, cfg.channels, cfg.bitrate)
            sender = UdpAudioSender(cfg)
        } catch (e: Exception) {
            Log.e(TAG, "编码器/发送器初始化失败", e)
            stopSelf()
            return
        }

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
        // 不指定 usage 即匹配所有可捕获播放（USAGE_MEDIA/USAGE_GAME/USAGE_UNKNOWN）。
        val captureConfig = AudioPlaybackCaptureConfiguration.Builder(projection).build()

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
            return
        }
        audioRecord = record
        record.startRecording()

        val pcm = ShortArray(frameSize * channels) // 960
        while (running) {
            val read = record.read(pcm, 0, pcm.size)
            if (read <= 0) continue
            if (read < pcm.size) continue // 不足一帧，丢弃
            try {
                val opus = encoder?.encode(pcm) ?: continue
                sender?.send(opus)
            } catch (e: Exception) {
                Log.w(TAG, "编码/发送异常", e)
            }
        }

        record.stop()
        record.release()
    }

    private fun stopCapture() {
        running = false
        captureThread?.join(500)
        captureThread = null
        encoder?.close()
        sender?.close()
        mediaProjection?.stop()
        stopForegroundCompat()
        stopSelf()
    }

    override fun onDestroy() {
        running = false
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
        const val EXTRA_RESULT_CODE = "result_code"
        const val EXTRA_RESULT_DATA = "result_data"
    }
}
