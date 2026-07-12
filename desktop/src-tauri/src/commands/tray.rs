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

/// 关闭窗口决策（H2：从 `handle_close_requested` 抽出的纯函数结果）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseDecision {
    /// 阻止关闭 + 隐藏窗口。
    Minimize,
    /// 异步 cleanup + 退出应用。
    Quit,
    /// 阻止关闭 + emit `close-requested` 给前端弹窗询问。
    Ask,
}

/// 根据 `config.close_action` 决定关闭窗口行为（H2：纯函数，便于单测）。
pub(crate) fn decide_close_action(action: &str) -> CloseDecision {
    match action {
        "minimize" => CloseDecision::Minimize,
        "quit" => CloseDecision::Quit,
        _ => CloseDecision::Ask,
    }
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
                // D3：异步调 cleanup_before_quit 再 exit，避免 block_on 死锁。
                let app_handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    let state: State<'_, AppState> = app_handle.state();
                    crate::commands::cleanup_before_quit(state.inner()).await;
                    app_handle.exit(0);
                });
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
        // H2：决策逻辑抽到纯函数 `decide_close_action`，便于单测。
        match decide_close_action(&action) {
            CloseDecision::Minimize => {
                api.prevent_close();
                let _ = window.hide();
            }
            CloseDecision::Quit => {
                // D3：异步调 cleanup_before_quit 再 exit。
                let app_handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    let state: State<'_, AppState> = app_handle.state();
                    crate::commands::cleanup_before_quit(state.inner()).await;
                    app_handle.exit(0);
                });
            }
            CloseDecision::Ask => {
                api.prevent_close();
                let _ = app.emit("close-requested", ());
            }
        }
    }
}

#[cfg(all(test, feature = "tauri_app"))]
mod tests {
    use super::*;

    #[test]
    fn decide_minimize() {
        assert_eq!(decide_close_action("minimize"), CloseDecision::Minimize);
    }

    #[test]
    fn decide_quit() {
        assert_eq!(decide_close_action("quit"), CloseDecision::Quit);
    }

    #[test]
    fn decide_ask() {
        assert_eq!(decide_close_action("ask"), CloseDecision::Ask);
    }

    #[test]
    fn decide_unknown_falls_back_to_ask() {
        assert_eq!(decide_close_action("foo"), CloseDecision::Ask);
    }

    #[test]
    fn decide_empty_falls_back_to_ask() {
        assert_eq!(decide_close_action(""), CloseDecision::Ask);
    }

    #[test]
    fn tray_menu_event_settings_serializes() {
        let e = TrayMenuEvent::Settings;
        let s = serde_json::to_string(&e).unwrap();
        assert_eq!(s, "{\"kind\":\"Settings\"}");
    }
}
