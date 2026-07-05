// SampleHandler.swift — 占位
//
// 职责：ReplayKit Broadcast Extension 入口。
// 继承 RPBroadcastSampleHandler，实现：
//   processSampleBuffer(_:with:) 处理 .audioApp 音频样本
//   broadcastStarted / broadcastPaused / broadcastResumed / broadcastFinished
// 将 CMSampleBuffer 交给 AudioProcessor 归一化，再经 OpusEncoderWrapper 编码、
// UdpAudioSender 发送。配对/密钥从 PairingStateReader (App Group) 读取。
//
// 约束：保持轻量，不放复杂 UI / 大缓存 / 重依赖。
// 详见 docs/First/03-audio-pipeline.md、08-platform-notes.md
