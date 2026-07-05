package com.soundlink.capture

// AudioCaptureService — 占位
//
// 职责：前台 Service，承载 MediaProjection + AudioPlaybackCapture。
// 流程：请求/持有 MediaProjection 授权 -> 构建 AudioPlaybackCaptureConfiguration
//        -> AudioRecord 读取 PCM(48kHz Stereo Int16) -> 交给 codec 编码 Opus
//        -> network 加密并 UDP 发送。
// 需展示前台通知，标注采集状态。API 29+。
// 详见 docs/First/03-audio-pipeline.md、08-platform-notes.md
