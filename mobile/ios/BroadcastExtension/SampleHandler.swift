// SampleHandler.swift
//
// ReplayKit Broadcast Upload Extension 入口。
// 流程：broadcastStarted 读 App Group 配置 → 初始化 processor/encoder/sender
//       → processSampleBuffer(.audioApp) 归一化 PCM → Opus 编码 → AEAD 加密 → UDP 发送
//       → broadcastFinished 发末包并清理。
//
// 约束：Extension 内存/生命周期受限，保持轻量；音频不出进程。
// 详见 docs/First/03-audio-pipeline.md、08-platform-notes.md §2。

import ReplayKit
import CoreMedia

class SampleHandler: RPBroadcastSampleHandler {

    private var processor: AudioProcessor?
    private var encoder: OpusEncoderWrapper?
    private var sender: UdpAudioSender?
    private var started = false

    override func broadcastStarted(withSetupInfo setupInfo: [String: NSObject]?) {
        // 在 Extension 进程读取主 App 写入的会话配置。
        guard let config = PairingStateReader.read() else {
            finishBroadcastWithError(NSError(
                domain: "SoundLink", code: 1,
                userInfo: [NSLocalizedDescriptionKey: "未找到会话配置，请先在主 App 配对并连接"]))
            return
        }
        guard let s = UdpAudioSender(config: config) else {
            finishBroadcastWithError(NSError(
                domain: "SoundLink", code: 2,
                userInfo: [NSLocalizedDescriptionKey: "UDP/密钥初始化失败"]))
            return
        }
        self.sender = s
        self.encoder = OpusEncoderWrapper(
            sampleRate: config.sampleRate,
            channels: config.channels,
            bitrate: config.bitrate)
        self.processor = AudioProcessor()
        self.started = true
    }

    override func processSampleBuffer(_ sampleBuffer: CMSampleBuffer, with sampleBufferType: RPSampleBufferType) {
        guard started, sampleBufferType == .audioApp else { return }

        // 1) 归一化为 48kHz/Stereo/Int16 交错 10ms 帧。
        guard let frames = processor?.process(sampleBuffer), !frames.isEmpty else { return }

        // 2) 逐帧 Opus 编码 → 加密 → UDP 发送。
        for (i, pcmFrame) in frames.enumerated() {
            guard let opus = encoder?.encode(pcmFrame) else { continue }
            let isLast = (i == frames.count - 1) && false // 正常帧不置 stream_end
            sender?.send(opusFrame: opus, streamEnd: isLast)
        }
    }

    override func broadcastPaused() { /* 无操作 */ }
    override func broadcastResumed() { /* 无操作 */ }

    override func broadcastFinished() {
        // 发送一个 stream_end 末包（空 Opus 帧或最后一帧置 flag）。
        if let s = sender {
            s.send(opusFrame: Data(), streamEnd: true)
        }
        processor?.reset()
        started = false
        PairingStateReader.clear()
    }
}
