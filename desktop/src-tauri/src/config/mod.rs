//! 配置读写（第一版：内存默认值，后续接 SQLite/JSON）。
//! 对齐 docs/First/07-tech-stack.md。

use crate::constants::{DEFAULT_AUDIO_PORT, DEFAULT_JITTER_MS};

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub audio_port: u16,
    pub jitter_ms: u32,
    pub default_output_device: Option<usize>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            audio_port: DEFAULT_AUDIO_PORT,
            jitter_ms: DEFAULT_JITTER_MS,
            default_output_device: None,
        }
    }
}
