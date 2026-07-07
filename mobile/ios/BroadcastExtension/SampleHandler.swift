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
import Foundation

class SampleHandler: RPBroadcastSampleHandler {

    private var processor: AudioProcessor?
    private var encoder: OpusEncoderWrapper?
    private var sender: UdpAudioSender?
    private var stopMonitor: DispatchSourceTimer?
    private var started = false

    /// 调试：是否转储采集 PCM + Opus 帧。由主 App 通过 App Group 写入。
    private var dumpEnabled = false
    /// 转储文件句柄（写入 Extension 共享容器目录）。
    private var pcmDumpFile: FileHandle?
    private var opusDumpFile: FileHandle?

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

        // 读取转储开关；启用时在共享容器创建 dump 文件。
        self.dumpEnabled = UserDefaults(suiteName: PairingStateReader.appGroupId)?
            .bool(forKey: "soundlink.dump_pcm") ?? false
        if dumpEnabled {
            openDumpFiles()
        }
        startStopMonitor()
    }

    override func processSampleBuffer(_ sampleBuffer: CMSampleBuffer, with sampleBufferType: RPSampleBufferType) {
        guard started, sampleBufferType == .audioApp else { return }

        // 1) 归一化为 48kHz/Stereo/Int16 交错 10ms 帧。
        guard let frames = processor?.process(sampleBuffer), !frames.isEmpty else { return }

        // 2) 逐帧 Opus 编码 → 加密 → UDP 发送。
        for (i, pcmFrame) in frames.enumerated() {
            // 转储采集后 PCM（编码前）。
            if dumpEnabled, let fh = pcmDumpFile {
                fh.write(pcmFrame)
            }
            guard let opus = encoder?.encode(pcmFrame) else { continue }
            // 转储 Opus 帧（4 字节小端长度前缀 + 数据）。
            if dumpEnabled, let fh = opusDumpFile {
                var len = UInt32(opus.count).littleEndian
                let lenData = withUnsafeBytes(of: &len) { Data($0) }
                fh.write(lenData)
                fh.write(opus)
            }
            let isLast = (i == frames.count - 1) && false // 正常帧不置 stream_end
            sender?.send(opusFrame: opus, streamEnd: isLast)
        }
    }

    override func broadcastPaused() { /* 无操作 */ }
    override func broadcastResumed() { /* 无操作 */ }

    override func broadcastFinished() {
        stopMonitor?.cancel()
        stopMonitor = nil
        // 发送一个 stream_end 末包（空 Opus 帧或最后一帧置 flag）。
        if let s = sender {
            s.send(opusFrame: Data(), streamEnd: true)
        }
        processor?.reset()
        started = false
        PairingStateReader.clear()
        closeDumpFiles()
    }

    private func startStopMonitor() {
        stopMonitor?.cancel()
        guard let defaults = UserDefaults(suiteName: PairingStateReader.appGroupId) else { return }
        let timer = DispatchSource.makeTimerSource(queue: DispatchQueue(label: "soundlink.stop.monitor"))
        timer.schedule(deadline: .now() + 1, repeating: 1)
        timer.setEventHandler { [weak self] in
            guard let self else { return }
            if defaults.bool(forKey: PairingStateReader.stopRequestedKey) {
                DispatchQueue.main.async {
                    self.finishBroadcastWithError(NSError(
                        domain: "SoundLink", code: 3,
                        userInfo: [NSLocalizedDescriptionKey: "控制连接已断开，广播已自动停止"]))
                }
            }
        }
        timer.resume()
        stopMonitor = timer
    }

    /// 在 App Group 共享容器创建 PCM / Opus 转储文件（覆盖写）。
    private func openDumpFiles() {
        guard let groupURL = FileManager.default.containerURL(
            forSecurityApplicationGroupIdentifier: PairingStateReader.appGroupId) else {
            return
        }
        let dumpDir = groupURL.appendingPathComponent("soundlink_dump", isDirectory: true)
        try? FileManager.default.createDirectory(at: dumpDir, withIntermediateDirectories: true)
        let pcmURL = dumpDir.appendingPathComponent("capture_pcm.raw")
        let opusURL = dumpDir.appendingPathComponent("capture_opus.bin")
        // 覆盖旧文件。
        try? FileManager.default.removeItem(at: pcmURL)
        try? FileManager.default.removeItem(at: opusURL)
        FileManager.default.createFile(atPath: pcmURL.path, contents: nil)
        FileManager.default.createFile(atPath: opusURL.path, contents: nil)
        pcmDumpFile = try? FileHandle(forWritingTo: pcmURL)
        opusDumpFile = try? FileHandle(forWritingTo: opusURL)
    }

    /// 关闭并释放转储文件句柄。
    private func closeDumpFiles() {
        try? pcmDumpFile?.close()
        try? opusDumpFile?.close()
        pcmDumpFile = nil
        opusDumpFile = nil
    }
}
