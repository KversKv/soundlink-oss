//! Windows 后端（CCD + GDI + 注册表 EDID + NVAPI + helper IPC）。

pub mod ccd;
pub mod device_restart;
pub mod dsc;
pub mod edid_reg;
pub mod gdi;
pub mod helper_client;
pub mod identify;
pub mod monitor_evt;
pub mod nvapi;

use crate::features::quick_resolution::model::{DisplayInfo, QrError, SystemMode};
use crate::features::quick_resolution::platform::{DisplayBackend, DisplaySnapshot};
use qr_ipc::MonitorKey;

pub struct WindowsBackend;

impl WindowsBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WindowsBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl DisplayBackend for WindowsBackend {
    fn enumerate(&self) -> Result<Vec<DisplayInfo>, QrError> {
        ccd::enumerate_displays()
    }

    fn enum_modes(&self, gdi_name: &str) -> Result<Vec<SystemMode>, QrError> {
        gdi::enum_modes(gdi_name)
    }

    fn apply(&self, gdi_name: &str, mode: &SystemMode) -> Result<(), QrError> {
        gdi::apply(gdi_name, mode)
    }

    fn snapshot(&self) -> Result<DisplaySnapshot, QrError> {
        let mut modes = Vec::new();
        for d in ccd::enumerate_displays()? {
            if let Some(cur) = &d.current {
                modes.push((d.gdi_name.clone(), *cur));
            }
        }
        Ok(DisplaySnapshot { modes })
    }

    fn restore(&self, snap: &DisplaySnapshot) -> Result<(), QrError> {
        // 尽力还原：单块失败不阻断其它块。
        let mut last_err: Option<QrError> = None;
        for (gdi_name, mode) in &snap.modes {
            if let Err(e) = gdi::apply(gdi_name, mode) {
                tracing::warn!("快照还原失败（{}）：{}", gdi_name, e);
                last_err = Some(e);
            }
        }
        match last_err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    fn read_edid(&self, key: &MonitorKey) -> Result<Vec<u8>, QrError> {
        edid_reg::read_effective_edid(&key.instance_path)
    }

    fn monitor_rect(&self, gdi_name: &str) -> Result<(i32, i32, u32, u32), QrError> {
        gdi::monitor_rect(gdi_name)
    }

    fn fullscreen_exclusive_active(&self) -> Option<String> {
        gdi::fullscreen_exclusive_process()
    }
}
