//! 非 Windows 平台后端：全部返回 Unsupported（display.md §一：本期仅 Windows）。

use crate::features::quick_resolution::model::{DisplayInfo, QrError, SystemMode};
use crate::features::quick_resolution::platform::{DisplayBackend, DisplaySnapshot};
use qr_ipc::MonitorKey;

pub struct StubBackend;

impl DisplayBackend for StubBackend {
    fn enumerate(&self) -> Result<Vec<DisplayInfo>, QrError> {
        Err(QrError::UnsupportedPlatform)
    }

    fn enum_modes(&self, _gdi_name: &str) -> Result<Vec<SystemMode>, QrError> {
        Err(QrError::UnsupportedPlatform)
    }

    fn apply(&self, _gdi_name: &str, _mode: &SystemMode) -> Result<(), QrError> {
        Err(QrError::UnsupportedPlatform)
    }

    fn snapshot(&self) -> Result<DisplaySnapshot, QrError> {
        Err(QrError::UnsupportedPlatform)
    }

    fn restore(&self, _snap: &DisplaySnapshot) -> Result<(), QrError> {
        Err(QrError::UnsupportedPlatform)
    }

    fn read_edid(&self, _key: &MonitorKey) -> Result<Vec<u8>, QrError> {
        Err(QrError::UnsupportedPlatform)
    }

    fn monitor_rect(&self, _gdi_name: &str) -> Result<(i32, i32, u32, u32), QrError> {
        Err(QrError::UnsupportedPlatform)
    }
}
