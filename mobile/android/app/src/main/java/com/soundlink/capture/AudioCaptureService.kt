package com.soundlink.capture

// AudioCaptureService — 占位
//
// 职责：前台 Service，承载 MediaProjection + AudioPlaybackCapture。
// 流程：请求/持有 MediaProjection 授权 -> 构建 AudioPlaybackCaptureConfiguration
//        -> AudioRecord 读取 PCM(48kHz Stereo Int16) -> 交给 codec 编码 Opus
//        -> network 加密并 UDP 发送。
// 需展示前台通知，标注采集状态。API 29+。
// 详见 docs/First/03-audio-pipeline.md、08-platform-notes.md
//
// ───────────────────────── 转发期间静音扬声器（必做） ─────────────────────────
// AudioPlaybackCapture 采集的是音量调节前的 PCM，因此把 STREAM_MUSIC 调到 0
// 不会影响转发，但可以让手机扬声器静音，避免「手机和电脑同时出声」。
// 用 [VolumeMuteController]（同包内）封装：
//
//   private val mute = VolumeMuteController(this)
//
//   override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
//       // ... startForeground(...)  先确保前台通知已展示
//       mute.muteMediaVolume()      // ← 在启动 MediaProjection 采集前调用
//       // ... 启动 AudioPlaybackCapture
//   }
//
//   override fun onDestroy() {
//       // ... 停止采集
//       mute.restoreMediaVolume()   // ← 必须恢复，否则用户得手动调回音量
//       super.onDestroy()
//   }
//
//   override fun onTaskRemoved(rootIntent: Intent?) {
//       mute.restoreMediaVolume()   // ← 用户划掉任务时也要恢复
//       super.onTaskRemoved(rootIntent)
//   }
//
// 异常崩溃恢复：如果 Service 进程被系统杀掉而非正常 onDestroy，
// 静音无法自动恢复。建议在主 App 进入前台时检查并恢复（调用 restoreMediaVolume()，
// 它是幂等的）。
