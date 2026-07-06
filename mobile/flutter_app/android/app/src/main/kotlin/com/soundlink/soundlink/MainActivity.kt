package com.soundlink.soundlink

import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel

class MainActivity : FlutterActivity() {
    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        // 注册 SoundLink 原生平台通道（采集控制 / 会话配置 / 设备 ID）。
        val plugin = SoundLinkPlugin(this)
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, SoundLinkPlugin.CHANNEL)
            .setMethodCallHandler(plugin)
    }
}
