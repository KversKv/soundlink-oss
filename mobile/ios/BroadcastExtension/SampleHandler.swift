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
    // N3：跟踪当前生效码率，检测 App Group pending 变化后热下发。
    private var currentBitrate = 0
    // 阶段 P：会话格式（编码器按此构造，基线帧转换后凑帧）。
    private var sessionRate = 48000
    private var sessionChannels = 2
    private var sessionFrameMs = 10
    // 会话格式样本累积缓冲（凑满一个会话帧编一次）。
    private var sessionAccum: [Int16] = []

    /// 调试：是否转储采集 PCM + Opus 帧。由主 App 通过 App Group 写入。
    private var dumpEnabled = false
    /// 转储文件句柄（写入 Extension 共享容器目录）。
    private var pcmDumpFile: FileHandle?
    private var opusDumpFile: FileHandle?

    override func broadcastStarted(withSetupInfo setupInfo: [String: NSObject]?) {
        // 在 Extension 进程读取主 App 写入的会话配置。
        guard let rawConfig = PairingStateReader.read() else {
            PairingStateReader.recordStopReason("broadcastStarted: 未找到会话配置")
            finishBroadcastWithError(NSError(
                domain: "SoundLink", code: 1,
                userInfo: [NSLocalizedDescriptionKey: "未找到会话配置，请先在主 App 配对并连接"]))
            return
        }
        // 阶段 P：按会话格式（白名单归一化）构造编码器/发送器。
        let config = rawConfig.sessionNormalized
        guard let s = UdpAudioSender(config: config) else {
            PairingStateReader.recordStopReason("broadcastStarted: UDP/密钥初始化失败")
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
        self.currentBitrate = config.bitrate
        self.sessionRate = config.sampleRate
        self.sessionChannels = config.channels
        self.sessionFrameMs = config.frameDurationMs
        self.sessionAccum = []
        UserDefaults(suiteName: PairingStateReader.appGroupId)?
            .removeObject(forKey: PairingStateReader.pendingBitrateKey)

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

        // N3：检测码率热调整请求（App Group 轮询）。
        if let defaults = UserDefaults(suiteName: PairingStateReader.appGroupId) {
            let pending = defaults.integer(forKey: PairingStateReader.pendingBitrateKey)
            if pending > 0 && pending != currentBitrate {
                encoder?.setBitrate(pending)
                currentBitrate = pending
                defaults.removeObject(forKey: PairingStateReader.pendingBitrateKey)
            }
        }

        // 1) 归一化为 48kHz/Stereo/Int16 交错 10ms 帧（基线）。
        guard let frames = processor?.process(sampleBuffer), !frames.isEmpty else { return }

        // 2) 基线帧 → 会话格式，累积凑满一个会话帧再编码。
        let sessionFrameLen = sessionRate / 1000 * sessionFrameMs * sessionChannels
        for pcmFrame in frames {
            // 转储采集后 PCM（编码前，基线格式）。
            if dumpEnabled, let fh = pcmDumpFile {
                fh.write(pcmFrame)
            }
            // Data(Int16 交错 LE) → [Int16]，转会话格式后累积。
            let baseline = pcmFrame.withUnsafeBytes { raw -> [Int16] in
                guard let base = raw.baseAddress?.assumingMemoryBound(to: Int16.self) else { return [] }
                return Array(UnsafeBufferPointer(start: base, count: pcmFrame.count / 2))
            }
            sessionAccum.append(contentsOf: SessionFormatConverter.toSession(
                baseline, sessionRate: sessionRate, sessionChannels: sessionChannels))

            // 凑满一个会话帧编一次（10ms 帧每次一拍；20ms 帧两拍）。
            while sessionAccum.count >= sessionFrameLen {
                let sessionFrame = Array(sessionAccum.prefix(sessionFrameLen))
                sessionAccum.removeFirst(sessionFrameLen)
                guard let opus = encoder?.encode(pcmDataFrom(sessionFrame)) else { continue }
                // 转储 Opus 帧（4 字节小端长度前缀 + 数据）。
                if dumpEnabled, let fh = opusDumpFile {
                    var len = UInt32(opus.count).littleEndian
                    let lenData = withUnsafeBytes(of: &len) { Data($0) }
                    fh.write(lenData)
                    fh.write(opus)
                }
                sender?.send(opusFrame: opus, streamEnd: false)
            }
        }
    }

    /// [Int16] → Int16 交错 LE Data（OpusEncoderWrapper.encode 输入）。
    private func pcmDataFrom(_ samples: [Int16]) -> Data {
        var out = Data(capacity: samples.count * 2)
        for s in samples {
            var le = s.littleEndian
            out.append(withUnsafeBytes(of: &le) { Data($0) })
        }
        return out
    }

    override func broadcastPaused() {
        // 暂停时清空 PCM 累积缓冲，避免恢复后用过期残留拼帧导致杂音/错位。
        processor?.reset()
    }
    override func broadcastResumed() {
        // 恢复时再次清空，确保从干净状态接收新 sample。
        // AudioProcessor 还会在输入格式变化时自动重建 converter。
        processor?.reset()
    }

    override func broadcastFinished() {
        stopMonitor?.cancel()
        stopMonitor = nil
        // 发送一个 stream_end 末包（空 Opus 帧或最后一帧置 flag）。
        if let s = sender {
            s.send(opusFrame: Data(), streamEnd: true)
        }
        processor?.reset()
        started = false
        PairingStateReader.recordStopReason("broadcastFinished: 广播正常结束（用户停止或系统终止）")
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
                PairingStateReader.recordStopReason("stop_monitor: 收到主 App stop_requested 信号")
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
