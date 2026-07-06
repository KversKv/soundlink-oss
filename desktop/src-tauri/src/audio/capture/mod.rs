//! 音频采集抽象层（阶段 5）。
//!
//! 统一 Sender 端采集源接口，与移动端采集组件协议一致：
//! 采集 → PCM 48kHz/Stereo/Int16 → Opus 编码 → 加密 → UDP 发送。
//!
//! 对齐 `docs/First/03-audio-pipeline.md` §1 桌面 Sender 链路。
//!
//! 实现源：
//! - [`sine::SineWaveCapture`]：440Hz 正弦测试源（跨平台，自测用）。
//! - `WASAPI Loopback`（Windows，`wasapi` feature 门控）：采集系统播放音频。
//! - `ScreenCaptureKit`（macOS，后续）：占位。

pub mod sine;

#[cfg(all(windows, feature = "wasapi"))]
pub mod wasapi_loopback;

#[cfg(target_os = "macos")]
pub mod macos_screencapturekit;

use crate::constants::{CHANNELS, FRAME_SAMPLES_TOTAL, SAMPLE_RATE};

/// 采集格式（基线 48kHz/Stereo/Int16 交错）。
#[derive(Debug, Clone, Copy)]
pub struct CaptureFormat {
    pub sample_rate: u32,
    pub channels: u16,
}

impl Default for CaptureFormat {
    fn default() -> Self {
        Self {
            sample_rate: SAMPLE_RATE,
            channels: CHANNELS as u16,
        }
    }
}

/// 采集源接口：拉取 10ms PCM 帧（交错 i16，长度 = FRAME_SAMPLES_TOTAL）。
///
/// 实现者负责将底层格式（如 WASAPI float32）归一化到 48kHz/Stereo/Int16。
/// `poll_frame` 在数据不足时返回 `None`，调用方按 10ms 节拍重试。
pub trait CaptureSource: Send {
    /// 源名称（如 "WASAPI Loopback"、"Sine 440Hz"）。
    fn name(&self) -> &str;

    /// 启动采集。
    fn start(&mut self) -> Result<(), String>;

    /// 停止采集。
    fn stop(&mut self);

    /// 拉取一帧 PCM（960 个 i16 交错样本）。无数据返回 None。
    fn poll_frame(&mut self) -> Option<Vec<i16>>;

    /// 是否正在采集。
    fn is_running(&self) -> bool;
}

/// 构造默认测试采集源：440Hz 正弦波。
pub fn default_test_source() -> Box<dyn CaptureSource> {
    Box::new(sine::SineWaveCapture::new(440.0, 0.25))
}

/// 每帧 PCM 样本数（交错）。
pub fn frame_pcm_len() -> usize {
    FRAME_SAMPLES_TOTAL
}
