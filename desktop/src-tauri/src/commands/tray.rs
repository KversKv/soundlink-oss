//! 系统托盘与关闭窗口行为。
//!
//! - [`setup_tray`]：在 `setup` 钩子中构建托盘图标 + 菜单。
//! - [`handle_close_requested`]：拦截窗口关闭，按 `config.close_action` 三分支处理。
//! - 菜单项「设置…」通过 emit 事件 `tray-menu-click` 通知前端切到设置页。

#![cfg(feature = "tauri_app")]

use crate::commands::AppState;
use tauri::{
    AppHandle, Emitter, Manager, State,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    WindowEvent,
};

/// 托盘菜单点击事件 payload（emit 给前端）。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind")]
pub enum TrayMenuEvent {
    /// 用户点击「设置…」
    Settings,
}

/// 在 setup 钩子中调用：构建托盘图标 + 菜单。
pub fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "设置…", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &settings, &sep, &quit])?;

    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| {
            tauri::Error::Anyhow(
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "缺少默认窗口图标",
                ))
                .into(),
            )
        })?;

    TrayIconBuilder::with_id("main-tray")
        .icon(icon)
        .tooltip("SoundLink")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => {
                show_main_window_inner(app);
            }
            "settings" => {
                show_main_window_inner(app);
                let _ = app.emit("tray-menu-click", TrayMenuEvent::Settings);
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(w) = app.get_webview_window("main") {
                    if w.is_visible().unwrap_or(false) {
                        let _ = w.hide();
                    } else {
                        let _ = w.show();
                        let _ = w.set_focus();
                    }
                }
            }
        })
        .build(app)?;
    Ok(())
}

fn show_main_window_inner(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// 处理窗口关闭请求（在 `on_window_event` 中调用）。
///
/// 三分支：
/// - `minimize`：阻止关闭 + 隐藏窗口
/// - `quit`：直接 `app.exit(0)`
/// - `ask`（默认）：阻止关闭 + emit `close-requested` 给前端弹窗
pub fn handle_close_requested(window: &tauri::Window, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event {
        let app = window.app_handle();
        let state: State<'_, AppState> = app.state();
        let action = state.config.lock().close_action.clone();
        match action.as_str() {
            "minimize" => {
                api.prevent_close();
                let _ = window.hide();
            }
            "quit" => {
                app.exit(0);
            }
            _ => {
                api.prevent_close();
                let _ = app.emit("close-requested", ());
            }
        }
    }
}
