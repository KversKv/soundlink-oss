// SoundLinkPlugin.kt
//
// Flutter MethodChannel("com.soundlink/platform") 的 Android 实现。
// 职责：写会话配置到 SharedPreferences、请求 MediaProjection 授权并启动采集 Service、
//       停止采集、读取设备 ID。
// 详见 docs/First/08-platform-notes.md §3、11-implementation-spec.md §8.3。

package com.soundlink.soundlink

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.media.projection.MediaProjectionManager
import androidx.activity.result.ActivityResultLauncher
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.ComponentActivity
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel
import com.soundlink.soundlink.capture.AudioCaptureService

class SoundLinkPlugin(
    private val activity: ComponentActivity,
) : MethodChannel.MethodCallHandler {

    private val prefs = activity.getSharedPreferences(AudioCaptureService.PREFS, Context.MODE_PRIVATE)
    private var pendingResult: MethodChannel.Result? = null

    private val projectionLauncher: ActivityResultLauncher<Intent> =
        activity.registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
            if (result.resultCode == Activity.RESULT_OK && result.data != null) {
                val svc = Intent(activity, AudioCaptureService::class.java).apply {
                    action = AudioCaptureService.ACTION_START
                    putExtra(AudioCaptureService.EXTRA_RESULT_CODE, result.resultCode)
                    putExtra(AudioCaptureService.EXTRA_RESULT_DATA, result.data)
                }
                activity.startForegroundService(svc)
                pendingResult?.success(true)
            } else {
                pendingResult?.error("DENIED", "MediaProjection 授权被拒绝", null)
            }
            pendingResult = null
        }

	override fun onMethodCall(call: MethodCall, result: MethodChannel.Result) {
        when (call.method) {
            "writeSessionConfig" -> {
                // Flutter 侧 SessionConfig.toJson() 返回 JSON 字符串，直接存。
                val rawJson = call.argument<String>("config") ?: ""
                prefs.edit().putString(AudioCaptureService.KEY_CONFIG, rawJson).apply()
                result.success(true)
            }
            "startCapture" -> {
                if (prefs.getString(AudioCaptureService.KEY_CONFIG, null) == null) {
                    result.error("NO_CONFIG", "请先配对并写入会话配置", null)
                    return
                }
                pendingResult = result
                try {
                    val mpm = activity.getSystemService(Context.MEDIA_PROJECTION_SERVICE) as MediaProjectionManager
                    projectionLauncher.launch(mpm.createScreenCaptureIntent())
                } catch (e: Exception) {
                    pendingResult = null
                    result.error("LAUNCH_FAIL", "无法请求 MediaProjection", e.message)
                }
            }
            "stopCapture" -> {
                val clearSession = call.argument<Boolean>("clearSession") ?: true
                val svc = Intent(activity, AudioCaptureService::class.java).apply {
                    action = AudioCaptureService.ACTION_STOP
                }
                activity.startService(svc)
                if (clearSession) {
                    prefs.edit().remove(AudioCaptureService.KEY_CONFIG).apply()
                }
                result.success(true)
            }
            "requestMediaProjection" -> result.success(true)
            "getDeviceId" -> result.success(getDeviceId())
            "setDumpPcm" -> {
                val enabled = call.argument<Boolean>("enabled") ?: false
                prefs.edit().putBoolean("dump_pcm", enabled).apply()
                result.success(true)
            }
            else -> result.notImplemented()
        }
    }

    private fun getDeviceId(): String {
        val existing = prefs.getString(DEVICE_ID_KEY, null)
        if (existing != null) return existing
        val id = "android-" + java.util.UUID.randomUUID().toString().take(8)
        prefs.edit().putString(DEVICE_ID_KEY, id).apply()
        return id
    }

    companion object {
        private const val TAG = "SoundLinkPlugin"
        const val CHANNEL = "com.soundlink/platform"
        private const val DEVICE_ID_KEY = "device_id"
    }
}
