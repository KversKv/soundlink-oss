// audio/opus_decoder.rs — 占位
//
// 职责：libopus 解码 48kHz Stereo；对丢失帧使用 PLC 补偿；输出 PCM 给 resampler。
//
// 说明：实际编解码逻辑（含 PLC）在 `opus_codec.rs` 的 `AudioCodec::decode_plc()`。
// 阶段 4 增强：连续 PLC 上限（PLC_CONSECUTIVE_LIMIT）在 `receiver.rs` 的
// `PlaybackFromJitter` 中限制，超过后切静音，避免 Opus PLC 持续衰减产生 artifacts。
// 此文件保留为模块占位，后续如需独立解码器抽象再扩展。
