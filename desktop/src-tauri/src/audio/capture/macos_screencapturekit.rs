//! macOS ScreenCaptureKit 采集（阶段 5，占位）。
//!
//! 对齐 `docs/First/08-platform-notes.md` §5：优先 ScreenCaptureKit Audio Capture，
//! 而非虚拟声卡（虚拟声卡需系统扩展/AudioServerPlugIn，签名/公证复杂）。
//!
//! 当前为占位实现：返回错误，待 macOS 环境实现 SCStream + 音频输出回调。

use super::CaptureSource;

/// macOS ScreenCaptureKit 采集源（占位）。
pub struct ScreenCaptureKitCapture {
    running: bool,
}

impl ScreenCaptureKitCapture {
    pub fn new() -> Self {
        Self { running: false }
    }
}

impl Default for ScreenCaptureKitCapture {
    fn default() -> Self {
        Self::new()
    }
}

impl CaptureSource for ScreenCaptureKitCapture {
    fn name(&self) -> &str {
        "ScreenCaptureKit (macOS, 未实现)"
    }

    fn start(&mut self) -> Result<(), String> {
        Err("ScreenCaptureKit 采集尚未实现（需 macOS 环境 + SCStream API）".into())
    }

    fn stop(&mut self) {
        self.running = false;
    }

    fn poll_frame(&mut self) -> Option<Vec<i16>> {
        None
    }

    fn is_running(&self) -> bool {
        self.running
    }
}
