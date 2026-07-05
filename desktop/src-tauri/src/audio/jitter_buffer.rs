// audio/jitter_buffer.rs — 占位
//
// 职责：按 timestamp/sequence 缓冲重排音频帧，吸收网络抖动。
// 三档：低延迟40ms / 平衡80ms(默认) / 稳定150ms；后续自适应。
// 输出稳定节奏帧给 opus_decoder。详见 docs/First/03-audio-pipeline.md
