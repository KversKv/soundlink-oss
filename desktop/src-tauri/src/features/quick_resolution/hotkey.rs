//! QR-1 全局热键（display.md §十四，默认关闭）。
//!
//! 仅 `settings.enable_global_hotkeys` 且 Pro 时注册；冲突即放弃该项并落日志。
//! 不使用低级键盘钩子（规避反作弊风险）。

use crate::features::quick_resolution::model::QuickResolutionSettings;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

/// 同步热键注册状态（设置变更/模式变更后调用）。
pub fn sync_hotkeys(app: &AppHandle, settings: &QuickResolutionSettings, pro_available: bool) {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    if !settings.enable_global_hotkeys || !pro_available {
        return;
    }
    for m in settings.modes.iter().filter(|m| m.state.is_ready()) {
        let Some(hk) = m.hotkey.as_deref() else { continue };
        let app2 = app.clone();
        let id = m.id.clone();
        if let Err(e) = gs.on_shortcut(hk, move |_, _, _| {
            let app3 = app2.clone();
            let id2 = id.clone();
            tauri::async_runtime::spawn(async move {
                let st: tauri::State<'_, crate::commands::AppState> = app3.state();
                if let Err(e) = st.qr.apply_by_id(&app3, &id2).await {
                    tracing::warn!("QR 热键切换失败：{}", e);
                }
            });
        }) {
            tracing::warn!("QR 热键 {} 注册失败（可能被占用）：{}", hk, e);
        }
    }
}
