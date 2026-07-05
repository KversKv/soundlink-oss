// audio/mod.rs — 占位
pub mod jitter_buffer;  // 抖动缓冲，三档(40/80/150ms)，后续自适应
pub mod opus_decoder;   // libopus 解码 + PLC 丢包补偿
pub mod resampler;      // 时钟漂移校正 / 软重采样
pub mod output;         // 平台音频输出后端
