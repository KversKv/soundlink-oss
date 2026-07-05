//! 跨平台音频输出设备统一模型。第一版直接复用 cpal 枚举（见 audio::output）。

pub use crate::audio::output::OutputDeviceInfo;

/// 列举输出设备。
pub fn list_output_devices() -> Vec<OutputDeviceInfo> {
    crate::audio::output::AudioOutput::new().list_devices()
}
