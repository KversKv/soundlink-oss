//! `qr_*` IPC 命令（display.md §10.3）。
//!
//! 门控（§十二）：每个命令第一行 `require_qr(state)` —
//! 唯一判据是 `ProCapabilities::quick_resolution_available()`（能力值门控，E4/E5/G6）。
//! 非 Pro 一律返回 `QrError::FeatureLocked`，前端统一转升级引导。

use crate::commands::AppState;
use crate::features::quick_resolution::model::*;
use tauri::{AppHandle, State};

/// 能力门控：后端唯一权威。
fn require_qr(state: &AppState) -> Result<(), QrError> {
    if state.caps.quick_resolution_available() {
        Ok(())
    } else {
        Err(QrError::FeatureLocked)
    }
}

/// 功能可用性探测（前端决定渲染完整区/遮罩/隐藏）。**不门控**——
/// 免费版也需要它来渲染「升级到 Pro」遮罩。
#[tauri::command]
pub fn qr_get_availability(state: State<'_, AppState>) -> QrAvailability {
    QrAvailability {
        available: state.caps.quick_resolution_available(),
        platform_supported: cfg!(windows),
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QrAvailability {
    /// Pro 能力是否可用（门控结论）。
    pub available: bool,
    /// 当前平台是否支持（仅 Windows）。
    pub platform_supported: bool,
}

#[tauri::command]
pub fn qr_get_displays(state: State<'_, AppState>) -> Result<Vec<DisplayInfo>, QrError> {
    require_qr(state.inner())?;
    state.qr.list_displays()
}

#[tauri::command]
pub fn qr_identify_displays(app: AppHandle, state: State<'_, AppState>) -> Result<(), QrError> {
    require_qr(state.inner())?;
    state.qr.identify(&app)
}

#[tauri::command]
pub fn qr_get_settings(state: State<'_, AppState>) -> Result<QuickResolutionSettings, QrError> {
    require_qr(state.inner())?;
    Ok(state.qr.settings())
}

#[tauri::command]
pub fn qr_set_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: QuickResolutionSettings,
) -> Result<QuickResolutionSettings, QrError> {
    require_qr(state.inner())?;
    let saved = state.qr.save_settings(settings)?;
    crate::features::quick_resolution::after_settings_changed(&app);
    Ok(saved)
}

#[tauri::command]
pub fn qr_list_modes(state: State<'_, AppState>) -> Result<Vec<DisplayModeEntry>, QrError> {
    require_qr(state.inner())?;
    Ok(state.qr.list_modes())
}

#[tauri::command]
pub fn qr_upsert_mode(
    app: AppHandle,
    state: State<'_, AppState>,
    entry: DisplayModeEntry,
) -> Result<DisplayModeEntry, QrError> {
    require_qr(state.inner())?;
    state.qr.upsert_mode(&app, entry)
}

#[tauri::command]
pub fn qr_delete_mode(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<(), QrError> {
    require_qr(state.inner())?;
    state.qr.delete_mode(&app, &id)
}

#[tauri::command]
pub fn qr_reorder_modes(
    app: AppHandle,
    state: State<'_, AppState>,
    ids: Vec<String>,
) -> Result<(), QrError> {
    require_qr(state.inner())?;
    state.qr.reorder_modes(&app, ids)
}

#[tauri::command]
pub fn qr_import_system_modes(
    app: AppHandle,
    state: State<'_, AppState>,
    target: ModeTarget,
) -> Result<Vec<DisplayModeEntry>, QrError> {
    require_qr(state.inner())?;
    state.qr.import_system_modes(&app, target)
}

#[tauri::command]
pub fn qr_validate_mode(
    state: State<'_, AppState>,
    draft: DisplayModeEntry,
) -> Result<ValidationReport, QrError> {
    require_qr(state.inner())?;
    Ok(state.qr.validate_mode(&draft))
}

#[tauri::command]
pub async fn qr_apply(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<SwitchResult, QrError> {
    require_qr(state.inner())?;
    let r = state.qr.apply_by_id(&app, &id).await;
    crate::features::quick_resolution::after_apply_attempt(&app);
    r
}

#[tauri::command]
pub async fn qr_apply_previous(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<SwitchResult, QrError> {
    require_qr(state.inner())?;
    let r = state.qr.apply_previous(&app).await;
    crate::features::quick_resolution::after_apply_attempt(&app);
    r
}

#[tauri::command]
pub fn qr_confirm_apply(state: State<'_, AppState>) -> Result<(), QrError> {
    require_qr(state.inner())?;
    state.qr.confirm_apply();
    Ok(())
}

#[tauri::command]
pub fn qr_revert_apply(state: State<'_, AppState>) -> Result<(), QrError> {
    require_qr(state.inner())?;
    state.qr.revert_apply();
    Ok(())
}

#[tauri::command]
pub fn qr_list_edid_backups(
    state: State<'_, AppState>,
    target: Option<ModeTarget>,
) -> Result<Vec<BackupInfo>, QrError> {
    require_qr(state.inner())?;
    Ok(state.qr.list_backups(target))
}

/// 安装 helper（唯一 UAC 入口，M4）。
#[tauri::command]
pub fn qr_install_helper(state: State<'_, AppState>) -> Result<(), QrError> {
    require_qr(state.inner())?;
    #[cfg(windows)]
    {
        crate::features::quick_resolution::platform::windows::helper_client::install_helper()
    }
    #[cfg(not(windows))]
    {
        Err(QrError::UnsupportedPlatform)
    }
}

/// helper 安装状态（计划任务是否已注册）。
#[tauri::command]
pub fn qr_helper_status(state: State<'_, AppState>) -> Result<bool, QrError> {
    require_qr(state.inner())?;
    #[cfg(windows)]
    {
        Ok(crate::features::quick_resolution::platform::windows::helper_client::helper_installed())
    }
    #[cfg(not(windows))]
    {
        Ok(false)
    }
}

/// DSC 状态 + 链路信息（诊断抽屉，M3）。
#[tauri::command]
pub fn qr_get_dsc_status(
    state: State<'_, AppState>,
    target: ModeTarget,
) -> Result<DscStatusPayload, QrError> {
    require_qr(state.inner())?;
    let (dsc, link) = state.qr.dsc_status(&target)?;
    Ok(DscStatusPayload { dsc, link })
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DscStatusPayload {
    pub dsc: DscState,
    pub link: Option<DisplayLinkInfo>,
}

/// 批量预置（M7）。空 ids = 预置全部 Draft/Validated。
#[tauri::command]
pub async fn qr_provision(
    app: AppHandle,
    state: State<'_, AppState>,
    ids: Vec<String>,
) -> Result<ProvisionReport, QrError> {
    require_qr(state.inner())?;
    state.qr.provision(&app, ids).await
}

/// 导出诊断包（M9）：打包近 14 天日志 + 能力档案 + EDID 备份 + 设置（去敏）到 zip。
#[tauri::command]
pub fn qr_export_diagnostics(state: State<'_, AppState>) -> Result<String, QrError> {
    require_qr(state.inner())?;
    state.qr.export_diagnostics()
}

/// 前端请求刷新模式状态（热插拔事件后）。
#[tauri::command]
pub fn qr_refresh_states(app: AppHandle, state: State<'_, AppState>) -> Result<(), QrError> {
    require_qr(state.inner())?;
    state.qr.refresh_states(&app);
    Ok(())
}
