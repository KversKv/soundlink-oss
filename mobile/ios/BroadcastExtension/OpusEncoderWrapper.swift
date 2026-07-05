// OpusEncoderWrapper.swift — 占位
//
// 职责：封装 libopus 编码器。参数：48kHz, Stereo, 10ms 帧, 128kbps 起步。
// 输入 PCM 帧，输出 Opus 编码字节，供打包/加密/发送。
// 支持后续码率自适应。
