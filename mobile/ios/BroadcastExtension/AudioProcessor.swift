// AudioProcessor.swift — 占位
//
// 职责：从 CMSampleBuffer 提取 AudioBufferList，转换为统一 PCM
// (48kHz / Stereo / Int16 或 Float32)。必要时做重采样与声道处理。
// 输出定长帧 (10ms) 供 OpusEncoderWrapper 编码。
