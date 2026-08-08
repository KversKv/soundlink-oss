//! 识别叠层（display.md §8.2）：每块屏幕中央显示巨大编号，3 秒后自动关闭。
//!
//! 实现：无边框/置顶/点击穿透的 Tauri 小窗，URL `index.html?view=qr-identify&n=<编号>`，
//! 由前端渲染数字本体（同一份 bundle，CSP 无新增面）。

use crate::features::quick_resolution::model::{DisplayInfo, QrError};
use crate::features::quick_resolution::platform::DisplayBackend;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

const OVERLAY_W: f64 = 200.0;
const OVERLAY_H: f64 = 160.0;
const SHOW_MS: u64 = 3000;

/// 为全部显示器弹出编号叠层。
pub fn show_identify_overlays(
    app: &tauri::AppHandle,
    backend: &dyn DisplayBackend,
    displays: &[DisplayInfo],
) -> Result<(), QrError> {
    // 先关旧叠层（连点防护）。
    close_overlays(app);
    let mut labels = Vec::new();
    for d in displays {
        let (mx, my, mw, mh) = backend.monitor_rect(&d.gdi_name)?;
        let cx = mx as f64 + mw as f64 / 2.0 - OVERLAY_W / 2.0;
        let cy = my as f64 + mh as f64 / 2.0 - OVERLAY_H / 2.0;
        let label = format!("qr-identify-{}", d.index);
        let url = format!("index.html?view=qr-identify&n={}", d.index);
        let win = WebviewWindowBuilder::new(app, &label, WebviewUrl::App(url.into()))
            .title("")
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .inner_size(OVERLAY_W, OVERLAY_H)
            .position(cx, cy)
            .build()
            .map_err(|e| QrError::Io(format!("创建识别叠层失败：{}", e)))?;
        let _ = win.set_ignore_cursor_events(true);
        labels.push(label);
    }
    // 到时自动关闭。
    let app2 = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(SHOW_MS));
        close_overlays(&app2);
    });
    Ok(())
}

/// 关闭全部识别叠层。
pub fn close_overlays(app: &tauri::AppHandle) {
    for (label, win) in app.webview_windows() {
        if label.starts_with("qr-identify-") {
            let _ = win.close();
        }
    }
}
