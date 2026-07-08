// AudioProcessor.swift
//
// 将 ReplayKit 的 CMSampleBuffer 归一化为 48kHz / Stereo / Int16 交错 PCM。
// 内部用 AVAudioConverter 做采样率与格式转换，并按 10ms 帧（960 样本）累积输出。
// 详见 docs/First/03-audio-pipeline.md、11-implementation-spec.md §1。

import Foundation
import AVFAudio
import CoreMedia

final class AudioProcessor {
    private let outSampleRate: Double = 48000
    private let outChannels: AVAudioChannelCount = 2
    private let samplesPerFrame: Int = 480 // 每帧每声道

    private var converter: AVAudioConverter?
    private var outFormat: AVAudioFormat!
    private var lastInputFormat: AVAudioFormat?
    private var pending = Data() // Int16 交错累积缓冲

    /// 处理一个音频 CMSampleBuffer，返回 0 或多个完整 10ms 帧（Int16 交错 Data）。
    func process(_ sampleBuffer: CMSampleBuffer) -> [Data] {
        guard let formatDesc = CMSampleBufferGetFormatDescription(sampleBuffer) else {
            return []
        }
        let inASBD = CMAudioFormatDescriptionGetStreamBasicDescription(formatDesc)
        guard let inBD = inASBD?.pointee else { return [] }

        var bd = inBD
        let inFormat = AVAudioFormat(streamDescription: &bd)!
        // 首次或输入格式变化（音频路由切换、暂停/恢复后采样率/声道变化）时
        // 重建 converter。否则沿用旧 converter 转换新格式数据会失败或产出垃圾。
        if converter == nil
            || lastInputFormat == nil
            || !inFormat.isEqual(lastInputFormat!) {
            outFormat = AVAudioFormat(
                commonFormat: .pcmFormatInt16,
                sampleRate: outSampleRate,
                channels: outChannels,
                interleaved: true)
            converter = AVAudioConverter(from: inFormat, to: outFormat)
            lastInputFormat = inFormat
            pending.removeAll() // 格式变化时丢弃过期残余，避免混帧
            if converter == nil { return [] }
        }

        let frameCount = CMSampleBufferGetNumSamples(sampleBuffer)
        guard frameCount > 0,
              let inFormat = converter?.inputFormat,
              let inputBuf = AVAudioPCMBuffer(pcmFormat: inFormat, frameCapacity: AVAudioFrameCount(frameCount)) else {
            return []
        }
        inputBuf.frameLength = AVAudioFrameCount(frameCount)
        let status = CMSampleBufferCopyPCMDataIntoAudioBufferList(
            sampleBuffer, at: 0, frameCount: Int32(frameCount), into: inputBuf.mutableAudioBufferList)
        guard status == noErr else { return [] }

        // 估算输出帧数（采样率比，+1 容差）。
        let ratio = outSampleRate / inFormat.sampleRate
        let outCapacity = AVAudioFrameCount(Double(frameCount) * ratio + 8)
        guard let outBuf = AVAudioPCMBuffer(pcmFormat: outFormat, frameCapacity: outCapacity) else {
            return []
        }

        var fed = false
        var convError: NSError?
        let convStatus = converter!.convert(to: outBuf, error: &convError) { _, outStatus in
            if fed {
                outStatus.pointee = .noDataNow
                return nil
            }
            fed = true
            outStatus.pointee = .haveData
            return inputBuf
        }
        if convStatus == .error || outBuf.frameLength == 0 { return [] }

        let byteCount = Int(outBuf.frameLength) * Int(outChannels) * MemoryLayout<Int16>.size
        var audioBuffer = outBuf.audioBufferList.pointee.mBuffers
        guard let data = audioBuffer.mData else { return [] }
        let availableBytes = min(byteCount, Int(audioBuffer.mDataByteSize))
        pending.append(Data(bytes: data, count: availableBytes))

        return drainFrames()
    }

    /// 从累积缓冲中切出所有完整的 960 样本（1920 字节）帧。
    private func drainFrames() -> [Data] {
        var frames = [Data]()
        let frameBytes = samplesPerFrame * Int(outChannels) * MemoryLayout<Int16>.size
        while pending.count >= frameBytes {
            let frame = pending.prefix(frameBytes)
            frames.append(Data(frame))
            pending.removeFirst(frameBytes)
        }
        return frames
    }

    /// 末尾刷新：无新样本时尝试丢弃残余（不足一帧）。
    func reset() {
        pending.removeAll()
    }
}
