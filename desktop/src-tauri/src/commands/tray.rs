//! 系统托盘与关闭窗口行为。
//!
//! - [`setup_tray`]：在 `setup` 钩子中构建托盘图标 + 菜单。
//! - [`handle_close_requested`]：拦截窗口关闭，按 `config.close_action` 三分支处理。
//! - 菜单项「设置…」通过 emit 事件 `tray-menu-click` 通知前端切到设置页。

#![cfg(feature = "tauri_app")]

use crate::commands::{tray_state_info, AppState};
use soundlink_pro_api::TrayItem;
use tauri::{
    AppHandle, Emitter, Manager, State,
    menu::{IsMenuItem, Menu, MenuItem, MenuItemKind, PredefinedMenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    WindowEvent, Wry,
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

/// 构建托盘菜单（MON-01 S15：菜单项来自 `caps.tray_items()` 能力驱动）。
/// 免费版仅「显示主窗口 / 设置… / 退出」；Pro 追加收发直控、静音与配置档子菜单。
fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let state: State<'_, AppState> = app.state();
    let info = tray_state_info(state.inner());
    let tray_items = state.caps.tray_items();

    let mut owned: Vec<MenuItemKind<Wry>> = Vec::new();
    owned.push(MenuItemKind::MenuItem(MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?));
    owned.push(MenuItemKind::MenuItem(MenuItem::with_id(app, "settings", "设置…", true, None::<&str>)?));

    if !tray_items.is_empty() {
        owned.push(MenuItemKind::Predefined(PredefinedMenuItem::separator(app)?));
        for item in &tray_items {
            match item {
                TrayItem::StartStopReceiver => {
                    let text = if info.receiver_running { "停止接收" } else { "开始接收" };
                    owned.push(MenuItemKind::MenuItem(MenuItem::with_id(app, "toggle-receiver", text, true, None::<&str>)?));
                }
                TrayItem::StartStopSender => {
                    let text = if info.sender_running { "停止发送" } else { "开始发送" };
                    owned.push(MenuItemKind::MenuItem(MenuItem::with_id(app, "toggle-sender", text, true, None::<&str>)?));
                }
                TrayItem::ToggleMute => {
                    let text = if info.muted { "取消静音" } else { "静音" };
                    owned.push(MenuItemKind::MenuItem(MenuItem::with_id(app, "toggle-mute", text, true, None::<&str>)?));
                }
                TrayItem::ProfileSwitcher => {
                    if info.profiles.is_empty() {
                        owned.push(MenuItemKind::MenuItem(MenuItem::with_id(
                            app,
                            "profiles-empty",
                            "切换到配置档（暂无配置档）",
                            false,
                            None::<&str>,
                        )?));
                    } else {
                        let mut sub_owned: Vec<MenuItemKind<Wry>> = Vec::new();
                        for p in &info.profiles {
                            let text = if info.active_profile.as_deref() == Some(p.id.as_str()) {
                                format!("✓ {}", p.name)
                            } else {
                                p.name.clone()
                            };
                            sub_owned.push(MenuItemKind::MenuItem(MenuItem::with_id(
                                app,
                                format!("profile:{}", p.id),
                                text,
                                true,
                                None::<&str>,
                            )?));
                        }
                        let refs: Vec<&dyn IsMenuItem<Wry>> =
                            sub_owned.iter().map(|k| k as &dyn IsMenuItem<Wry>).collect();
                        owned.push(MenuItemKind::Submenu(Submenu::with_items(
                            app,
                            "切换到配置档",
                            true,
                            &refs,
                        )?));
                    }
                }
            }
        }
    }

    // QR-1：快速分辨率切换二级菜单（Pro + enabled + showInTray 时出现）。
    if let Some(sub) = crate::features::quick_resolution::tray::build_qr_submenu(app) {
        owned.push(MenuItemKind::Predefined(PredefinedMenuItem::separator(app)?));
        owned.push(MenuItemKind::Submenu(sub));
    }

    owned.push(MenuItemKind::Predefined(PredefinedMenuItem::separator(app)?));
    owned.push(MenuItemKind::MenuItem(MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?));

    let refs: Vec<&dyn IsMenuItem<Wry>> = owned.iter().map(|k| k as &dyn IsMenuItem<Wry>).collect();
    Menu::with_items(app, &refs)
}

/// 托盘提示文字随运行状态更新（如「SoundLink · 接收中」）。
fn tooltip_for(info: &crate::commands::TrayStateInfo) -> String {
    match (info.receiver_running, info.sender_running) {
        (true, false) => "SoundLink · 接收中".into(),
        (false, true) => "SoundLink · 发送中".into(),
        (true, true) => "SoundLink · 收发中".into(),
        (false, false) => "SoundLink".into(),
    }
}

/// 状态变化后重建托盘菜单（文字翻转）与提示。
///
/// QR-1（display.md §十一）：菜单重建 200ms 防抖，避免热插拔/状态连发导致闪烁。
pub fn refresh_tray(app: &AppHandle) {
    use std::sync::atomic::{AtomicBool, Ordering};
    static PENDING: AtomicBool = AtomicBool::new(false);
    if PENDING.swap(true, Ordering::SeqCst) {
        return; // 已有在途重建
    }
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        PENDING.store(false, Ordering::SeqCst);
        if let Ok(menu) = build_menu(&app2) {
            if let Some(tray) = app2.tray_by_id("main-tray") {
                let _ = tray.set_menu(Some(menu));
                let state: State<'_, AppState> = app2.state();
                let info = tray_state_info(state.inner());
                let _ = tray.set_tooltip(Some(&tooltip_for(&info)));
            }
        }
    });
}

/// 立即重建托盘菜单（setup 首次构建用，不走防抖）。
pub fn refresh_tray_now(app: &AppHandle) {
    if let Ok(menu) = build_menu(app) {
        if let Some(tray) = app.tray_by_id("main-tray") {
            let _ = tray.set_menu(Some(menu));
            let state: State<'_, AppState> = app.state();
            let info = tray_state_info(state.inner());
            let _ = tray.set_tooltip(Some(&tooltip_for(&info)));
        }
    }
}

/// 在 setup 钩子中调用：构建托盘图标 + 菜单。
pub fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let menu = build_menu(app.handle())?;
    let tooltip = {
        let state: State<'_, AppState> = app.state();
        tooltip_for(&tray_state_info(state.inner()))
    };

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
        .tooltip(tooltip)
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
            // MON-01 S15：收发直控（不打开主窗口即可完成开始→停止全流程）。
            "toggle-receiver" => {
                let app_handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = crate::commands::toggle_receiver_inner(app_handle.clone()).await {
                        tracing::warn!("托盘切换接收失败：{}", e);
                    }
                    refresh_tray(&app_handle);
                });
            }
            "toggle-sender" => {
                let app_handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = crate::commands::toggle_sender_inner(app_handle.clone()).await {
                        tracing::warn!("托盘切换发送失败：{}", e);
                        // 无可连接设备时显示主窗口，引导用户手动操作。
                        show_main_window_inner(&app_handle);
                    }
                    refresh_tray(&app_handle);
                });
            }
            "toggle-mute" => {
                let state: State<'_, AppState> = app.state();
                crate::commands::toggle_mute_inner(state.inner());
                refresh_tray(app);
            }
            // QR-1：托盘快切（只走 Apply 快路径，§十一）。
            id if id.starts_with("qr_apply::") => {
                let mode_id = id.trim_start_matches("qr_apply::").to_string();
                let app_handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    let state: State<'_, AppState> = app_handle.state();
                    match state.qr.apply_by_id(&app_handle, &mode_id).await {
                        Ok(r) => tracing::info!("QR 托盘切换 {:?}：{:?}", mode_id, r),
                        Err(e) => {
                            tracing::warn!("QR 托盘切换失败：{}", e);
                            notify(&app_handle, "分辨率切换失败", &e.to_string());
                        }
                    }
                    refresh_tray(&app_handle);
                });
            }
            "qr_restore_prev" => {
                let app_handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    let state: State<'_, AppState> = app_handle.state();
                    if let Err(e) = state.qr.apply_previous(&app_handle).await {
                        tracing::warn!("QR 恢复上一个失败：{}", e);
                        notify(&app_handle, "恢复上一个分辨率失败", &e.to_string());
                    }
                    refresh_tray(&app_handle);
                });
            }
            "qr_pending" | "qr_manage" => {
                show_main_window_inner(app);
                let _ = app.emit("tray-menu-click", TrayMenuEvent::Settings);
            }
            id if id.starts_with("profile:") => {
                let profile_id = id.trim_start_matches("profile:").to_string();
                let state: State<'_, AppState> = app.state();
                match crate::commands::apply_profile(state, profile_id) {
                    Ok(r) => {
                        tracing::info!("托盘应用配置档：{}（重启流生效={}）", r.profile.name, r.restart_required);
                        let _ = app.emit(
                            "profile-applied",
                            serde_json::json!({
                                "id": r.profile.id,
                                "name": r.profile.name,
                                "restart_required": r.restart_required,
                            }),
                        );
                    }
                    Err(e) => tracing::warn!("托盘应用配置档失败：{}", e),
                }
                refresh_tray(app);
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

/// setup 完成后的托盘自检（M2 验证 QR 子菜单接入，避免静默缺失）。
pub fn verify_setup(app: &AppHandle) {
    let has_qr = crate::features::quick_resolution::tray::build_qr_submenu(app).is_some();
    tracing::debug!("QR 托盘子菜单构建自检：{}", if has_qr { "有内容" } else { "未展示（未启用/免费版）" });
}

fn show_main_window_inner(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// 托盘操作失败走系统通知（§十一：不强行弹主窗打断用户）。
/// 非 Windows 平台为空操作。
#[cfg(windows)]
fn notify(_app: &AppHandle, title: &str, body: &str) {
    use std::process::Command;
    let script = format!(
        "Add-Type -AssemblyName System.Windows.Forms;\
         $n = New-Object System.Windows.Forms.NotifyIcon;\
         $n.Icon = [System.Drawing.SystemIcons]::Information;\
         $n.Visible = $true;\
         $n.BalloonTipTitle = '{}';\
         $n.BalloonTipText = '{}';\
         $n.ShowBalloonTip(4000);\
         Start-Sleep -Milliseconds 4500;\
         $n.Dispose()",
        title.replace('\'', "''"),
        body.replace('\'', "''")
    );
    let _ = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", &script])
        .spawn();
}

/// 非 Windows：托盘失败仅落日志。
#[cfg(not(windows))]
fn notify(_app: &AppHandle, title: &str, body: &str) {
    tracing::warn!("QR 托盘通知（{}）：{}", title, body);
}

/// 处理窗口关闭请求（在 `on_window_event` 中调用）。
///
/// 三分支：
/// - `minimize`：阻止关闭 + 隐藏窗口
/// - `quit`：直接 `app.exit(0)`
/// - `ask`（默认）：阻止关闭 + emit `close-requested` 给前端弹窗
pub fn handle_close_requested(window: &tauri::Window, event: &WindowEvent) {
    // QR-1 子窗（确认窗/识别叠层）直接放行，不拦截（display.md §8.2/§7.4）。
    let label = window.label();
    if label == "qr-confirm" || label.starts_with("qr-identify-") {
        return;
    }
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
