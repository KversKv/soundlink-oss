//! QR-1 托盘二级菜单（display.md §十一）。
//!
//! 工程要点：只列 Ready 模式（未预置不可点）；多屏按显示器分组；
//! 当前生效项用 ✓ 前缀标记；末尾恒有「恢复上一个 / 管理…」。
//! 重建触发与防抖由 `commands::tray::refresh_tray` 统一承载（200ms debounce 在 M2 接线处）。

use crate::commands::AppState;
use crate::features::quick_resolution::model::{DisplayModeEntry, ModeState, ModeTarget};
use std::collections::BTreeMap;
use tauri::menu::{CheckMenuItem, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Manager, State, Wry};

/// 构建「快速分辨率切换」二级菜单。返回 None 表示不展示（未启用/无 Pro/无托盘项）。
pub fn build_qr_submenu(app: &AppHandle) -> Option<Submenu<Wry>> {
    let state: State<'_, AppState> = app.state();
    // 能力门控：免费版不出现 QR 子菜单（§十二）。
    if !state.caps.quick_resolution_available() {
        return None;
    }
    let st = state.qr.settings();
    if !st.enabled || !st.show_in_tray {
        return None;
    }

    // 当前每块显示器生效模式（gdi_name → "WxH@Hz"）。
    let current: std::collections::HashMap<u32, String> = state
        .qr
        .list_displays()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|d| {
            d.current
                .map(|c| (d.index, format!("{}×{} @{}Hz", c.width, c.height, c.refresh_hz)))
        })
        .collect();

    let mut items: Vec<Box<dyn tauri::menu::IsMenuItem<Wry>>> = Vec::new();

    // 当前行（禁用）。
    let head_text = if current.len() == 1 {
        format!("当前: {}", current.values().next().cloned().unwrap_or_default())
    } else {
        let mut parts: Vec<String> = current
            .iter()
            .map(|(i, t)| format!("显示器{}: {}", i, t))
            .collect();
        parts.sort();
        format!("当前: {}", parts.join(" | "))
    };
    items.push(Box::new(
        MenuItem::with_id(app, "qr_cur", head_text, false, None::<&str>).ok()?,
    ));
    items.push(Box::new(PredefinedMenuItem::separator(app).ok()?));

    // 按显示器分组（显示器编号 → Ready+pinned 模式）。
    let mut by_display: BTreeMap<u32, Vec<&DisplayModeEntry>> = BTreeMap::new();
    for m in st.modes.iter().filter(|m| m.pinned_to_tray && m.state.is_ready()) {
        let idx = display_index_of(m, &state);
        by_display.entry(idx).or_default().push(m);
    }
    let multi = by_display.len() > 1;
    let mut shown = 0usize;
    'outer: for (idx, modes) in &by_display {
        if multi {
            items.push(Box::new(
                MenuItem::with_id(app, format!("qr_hdr_{}", idx), format!("— 显示器 {} —", idx), false, None::<&str>).ok()?,
            ));
        }
        for m in modes {
            if shown >= st.max_tray_items as usize {
                break 'outer;
            }
            let cur_text = current.get(idx).cloned().unwrap_or_default();
            let checked = cur_text == format!("{}×{} @{}Hz", m.width, m.height, m.refresh_hz);
            let label = format!("{}  ({}×{} @{}Hz)", m.label, m.width, m.height, m.refresh_hz);
            let item = CheckMenuItem::with_id(
                app,
                format!("qr_apply::{}", m.id),
                label,
                true,
                checked,
                None::<&str>,
            )
            .ok()?;
            items.push(Box::new(item));
            shown += 1;
        }
    }

    // 待预置提示（禁用项，引导去面板）。
    if st.modes.iter().any(|m| matches!(m.state, ModeState::Validated | ModeState::Draft)) {
        items.push(Box::new(PredefinedMenuItem::separator(app).ok()?));
        items.push(Box::new(
            MenuItem::with_id(app, "qr_pending", "有模式待预置，点击前往设置", true, None::<&str>).ok()?,
        ));
    }

    items.push(Box::new(PredefinedMenuItem::separator(app).ok()?));
    items.push(Box::new(
        MenuItem::with_id(app, "qr_restore_prev", "恢复上一个分辨率", true, None::<&str>).ok()?,
    ));
    items.push(Box::new(
        MenuItem::with_id(app, "qr_manage", "管理分辨率列表…", true, None::<&str>).ok()?,
    ));

    let refs: Vec<&dyn tauri::menu::IsMenuItem<Wry>> =
        items.iter().map(|b| b.as_ref() as &dyn tauri::menu::IsMenuItem<Wry>).collect();
    Submenu::with_items(app, "快速分辨率切换", true, &refs).ok()
}

/// 模式所属显示器编号（解析 target；失败归 0）。
fn display_index_of(m: &DisplayModeEntry, state: &AppState) -> u32 {
    match &m.target {
        ModeTarget::Primary => 1,
        ModeTarget::Index { index } => *index,
        ModeTarget::Key { key } => state
            .qr
            .list_displays()
            .unwrap_or_default()
            .into_iter()
            .find(|d| &d.key == key)
            .map(|d| d.index)
            .unwrap_or(1),
    }
}
