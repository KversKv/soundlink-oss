package com.soundlink.capture

import android.content.Context
import android.media.AudioManager
import android.util.Log

/**
 * VolumeMuteController — 转发期间静音手机扬声器媒体音量。
 *
 * 背景：Android AudioPlaybackCapture 采集的是 AudioFlinger 中应用提交的
 * 原始 PCM（音量调节前），因此把 STREAM_MUSIC 调到 0 不会影响转发，
 * 但可以让手机扬声器静音，达到「音频只转到电脑」的效果。
 *
 * 用法（在 AudioCaptureService 中）：
 *   private val mute = VolumeMuteController(this)
 *
 *   override fun onStartCommand(...): Int {
 *       mute.muteMediaVolume()      // 启动采集前
 *       // ... 启动 MediaProjection + AudioPlaybackCapture
 *   }
 *
 *   override fun onDestroy() {
 *       // ... 停止采集
 *       mute.restoreMediaVolume()   // 一定要恢复，否则用户得手动调
 *       super.onDestroy()
 *   }
 *
 *   // 处理异常崩溃：Service 被系统杀掉时 onTaskRemoved 是最后机会
 *   override fun onTaskRemoved(rootIntent: Intent?) {
 *       mute.restoreMediaVolume()
 *       super.onTaskRemoved(rootIntent)
 *   }
 *
 * 注意事项：
 * - 采集期间不要让用户去调音量，否则 savedVolume 就过时了；
 * - 如果用户在采集期间手动调了音量，恢复时可能不是用户期望的值，
 *   可以考虑监听音量变化广播（ACTION_VOLUME_CHANGED）动态更新 savedVolume，
 *   但第一版不引入这个复杂度；
 * - 部分定制 ROM（华为/小米老版本）可能在 STREAM_MUSIC=0 时也阻断采集，
 *   如发现此问题，需要改用「插入耳机」方案。
 */
class VolumeMuteController(private val context: Context) {

    companion object {
        private const val TAG = "VolMuteCtrl"
        /** 标记未保存过音量（避免误把 0 当成保存值）。 */
        private const val NOT_SAVED = -1
    }

    private var savedMusicVolume: Int = NOT_SAVED
    private val audioManager: AudioManager
        get() = context.getSystemService(Context.AUDIO_SERVICE) as AudioManager

    /**
     * 静音 STREAM_MUSIC：保存当前音量 → 设为 0。
     * 幂等：多次调用只会保存第一次的值。
     */
    fun muteMediaVolume() {
        if (savedMusicVolume != NOT_SAVED) {
            Log.d(TAG, "已静音过，跳过（saved=${savedMusicVolume}）")
            return
        }
        val cur = audioManager.getStreamVolume(AudioManager.STREAM_MUSIC)
        if (cur <= 0) {
            // 已经是 0：不保存，避免退出时把用户本来的 0 误恢复成其他值。
            Log.d(TAG, "当前媒体音量已为 0，无需静音")
            return
        }
        savedMusicVolume = cur
        audioManager.setStreamVolume(AudioManager.STREAM_MUSIC, 0, 0)
        Log.i(TAG, "媒体音量静音：$cur -> 0")
    }

    /**
     * 恢复 STREAM_MUSIC 到 muteMediaVolume() 之前保存的值。
     * 幂等：多次调用安全；若未保存过或当时已是 0，则不做任何事。
     */
    fun restoreMediaVolume() {
        if (savedMusicVolume == NOT_SAVED) {
            return
        }
        // 如果用户在采集期间又调过音量，我们仍然恢复到 savedMusicVolume。
        // 这是设计取舍：第一版优先保证「恢复到非 0」，避免用户忘记调回来。
        // 如果你的产品想做「保留用户采集期间的手动调整」，可以在此读取当前值
        // 并对比，若当前值非 0 则跳过恢复。
        audioManager.setStreamVolume(AudioManager.STREAM_MUSIC, savedMusicVolume, 0)
        Log.i(TAG, "媒体音量恢复：0 -> $savedMusicVolume")
        savedMusicVolume = NOT_SAVED
    }

    /** 当前是否处于已静音状态（已保存但未恢复）。 */
    fun isMuted(): Boolean = savedMusicVolume != NOT_SAVED

    /** 保存的原始音量（未保存时返回 -1）。 */
    fun savedVolume(): Int = savedMusicVolume
}
