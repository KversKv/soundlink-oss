package com.soundlink.codec

// OpusEncoder — 占位
//
// 职责：封装 libopus 编码（JNI 或成熟 Opus 库）。
// 参数：48kHz, Stereo, 10ms 帧, 128kbps 起步，支持后续码率自适应。
// 输入 PCM 帧，输出 Opus 字节。
