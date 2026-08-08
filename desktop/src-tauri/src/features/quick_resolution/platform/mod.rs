//! 平台抽象层（display.md §三：`DisplayBackend` trait 保留多 GPU/多 OS 插拔能力）。
//!
//! 本期仅实现 NVIDIA-on-Windows（`windows` 模块）；非 Windows 走 [`stub`]。

#[cfg(windows)]
pub mod windows;
#[cfg(not(windows))]
pub mod stub;

use crate::features::quick_resolution::model::{DisplayInfo, QrError, SystemMode};
use qr_ipc::MonitorKey;

/// 完整拓扑快照（回滚用）：每块显示器的当前模式。
#[derive(Debug, Clone, Default)]
pub struct DisplaySnapshot {
    /// (gdi_name, 模式)。
    pub modes: Vec<(String, SystemMode)>,
}

/// 显示后端：枚举/切换/快照/EDID 读取。
///
/// GPU 相关能力（NVAPI 自定义分辨率、DSC 细节）在 Windows 实现内部
/// 通过 feature probe 暴露，不进 trait（display.md §一：本期仅 NVIDIA）。
pub trait DisplayBackend: Send + Sync {
    /// 枚举活动显示器（编号 = CCD source id 排序）。
    fn enumerate(&self) -> Result<Vec<DisplayInfo>, QrError>;

    /// 枚举某显示器系统已注册模式（GDI）。
    fn enum_modes(&self, gdi_name: &str) -> Result<Vec<SystemMode>, QrError>;

    /// 快切：CDS_TEST → CDS_UPDATEREGISTRY。
    fn apply(&self, gdi_name: &str, mode: &SystemMode) -> Result<(), QrError>;

    /// 拓扑快照（回滚保险）。
    fn snapshot(&self) -> Result<DisplaySnapshot, QrError>;

    /// 还原快照。
    fn restore(&self, snap: &DisplaySnapshot) -> Result<(), QrError>;

    /// 读取显示器 EDID（override 优先，其次原生；HKLM 只读不需提权）。
    fn read_edid(&self, key: &MonitorKey) -> Result<Vec<u8>, QrError>;

    /// 显示器屏幕矩形（识别叠层定位用）：(x, y, w, h)。
    fn monitor_rect(&self, gdi_name: &str) -> Result<(i32, i32, u32, u32), QrError>;

    /// DPI 缩放因子（识别叠层物理→逻辑坐标换算）。默认 1.0。
    fn scale_factor_of(&self, _gdi_name: &str) -> Option<f64> {
        None
    }

    /// 检测是否有全屏独占程序在跑（预置前置守卫，display.md §7.3 step 0）。
    fn fullscreen_exclusive_active(&self) -> Option<String> {
        None
    }
}

/// 构造当前平台后端。
pub fn default_backend() -> Box<dyn DisplayBackend> {
    #[cfg(windows)]
    {
        Box::new(windows::WindowsBackend::new())
    }
    #[cfg(not(windows))]
    {
        Box::new(stub::StubBackend)
    }
}

/// 启动显示器热插拔/模式变更监听（非 Windows 为空操作）。
#[cfg(windows)]
pub fn start_display_hook(app: tauri::AppHandle) {
    windows::monitor_evt::start_display_change_hook(app);
}

/// 启动显示器热插拔/模式变更监听（非 Windows 为空操作）。
#[cfg(not(windows))]
pub fn start_display_hook(_app: tauri::AppHandle) {}
