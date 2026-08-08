//! QR-1 分辨率快速切换（Pro 能力，display.md）。
//!
//! 两阶段模型：**Provision 预置**（低频、提权，把自定义模式注入系统列表）
//! 与 **Apply 快切**（高频、免提权，`ChangeDisplaySettingsEx` 毫秒级生效）。
//!
//! 门控：唯一判据是 `ProCapabilities::quick_resolution_available()`（E4/E5），
//! 本模块不出现任何 `is_pro()` 判断。
//!
//! 模块地图：
//! - [`model`]：数据模型 + [`QrError`]
//! - [`store`]：持久化（`quick_resolution.json` / 能力档案 / EDID 备份 / 崩溃恢复标记）
//! - [`platform`]：`DisplayBackend` trait + Windows 实现（CCD/GDI/NVAPI/helper IPC）
//! - [`service`]：门面 + 串行锁 + 事件广播
//! - [`applier`]：快切；[`rollback`]：15s 确认回滚 + 启动自检
//! - [`capability`]：自适应能力探测阶梯；[`provisioner`]：批量预置编排
//! - [`tray`]：托盘二级菜单；[`hotkey`]：全局热键（默认关）
//! - [`commands`]：全部 `qr_*` IPC 命令

pub mod applier;
pub mod capability;
pub mod commands;
#[cfg(all(windows, feature = "tauri_app"))]
pub mod helper_core;
pub mod hotkey;
pub mod model;
pub mod platform;
pub mod provisioner;
pub mod rollback;
pub mod service;
pub mod store;
pub mod tray;

pub use model::{QrError, QuickResolutionSettings};
pub use service::QrService;

/// 设置/模式变更后钩子：重建托盘菜单（M2 起含 QR 子菜单）+ 同步热键（M9）。
pub(crate) fn after_settings_changed(app: &tauri::AppHandle) {
    use tauri::Manager;
    crate::commands::tray::refresh_tray(app);
    let st: tauri::State<'_, crate::commands::AppState> = app.state();
    let settings = st.qr.settings();
    hotkey::sync_hotkeys(app, &settings, st.caps.quick_resolution_available());
}

/// 切换尝试后钩子：托盘当前项标记需要更新。
pub(crate) fn after_apply_attempt(app: &tauri::AppHandle) {
    crate::commands::tray::refresh_tray(app);
}
