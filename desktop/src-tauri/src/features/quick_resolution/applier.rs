//! Applier：高频快切路径（display.md §7.1「必须快且稳」）。

use crate::features::quick_resolution::model::{
    DisplayModeEntry, ModeTarget, QrError, SystemMode,
};
use crate::features::quick_resolution::platform::DisplayBackend;
use qr_ipc::MonitorKey;

/// 解析后的目标显示器（GDI 名为运行时解析结果）。
#[derive(Debug, Clone)]
pub struct ResolvedDisplay {
    pub index: u32,
    pub key: MonitorKey,
    pub gdi_name: String,
    pub friendly_name: String,
}

/// ModeTarget → 运行时显示器（每次解析，GDI 名不持久化）。
pub fn resolve_target(
    backend: &dyn DisplayBackend,
    target: &ModeTarget,
) -> Result<ResolvedDisplay, QrError> {
    let displays = backend.enumerate()?;
    let found = match target {
        ModeTarget::Primary => displays.iter().find(|d| d.is_primary),
        ModeTarget::Index { index } => displays.iter().find(|d| d.index == *index),
        ModeTarget::Key { key } => displays.iter().find(|d| &d.key == key),
    };
    let d = found.ok_or_else(|| QrError::DisplayNotFound(format!("{:?}", target)))?;
    Ok(ResolvedDisplay {
        index: d.index,
        key: d.key.clone(),
        gdi_name: d.gdi_name.clone(),
        friendly_name: d.friendly_name.clone(),
    })
}

/// 快切：模式必须已在系统列表（Ready），否则 `ModeNotRegistered`。
pub fn apply(
    backend: &dyn DisplayBackend,
    target: &ResolvedDisplay,
    m: &DisplayModeEntry,
) -> Result<(), QrError> {
    let mode = SystemMode {
        width: m.width,
        height: m.height,
        refresh_hz: m.refresh_hz,
        bits_per_pel: 32,
    };
    backend.apply(&target.gdi_name, &mode)
}
