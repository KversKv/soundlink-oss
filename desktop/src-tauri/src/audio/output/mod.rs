// audio/output/mod.rs — 占位
//
// 职责：跨平台音频输出后端抽象（枚举设备、选择设备、低延迟播放、音量、插拔）。
#[cfg(target_os = "windows")]
pub mod windows_wasapi;
#[cfg(target_os = "macos")]
pub mod macos_coreaudio;
#[cfg(target_os = "linux")]
pub mod linux_pipewire;
