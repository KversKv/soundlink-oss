//! 识别叠层（display.md §8.2）：每块屏幕中央显示巨大编号，3 秒后自动关闭。
//!
//! 实现：无边框/置顶/点击穿透的 Tauri 小窗，URL `index.html?view=qr-identify&n=<编号>`，
//! 由前端渲染数字本体（同一份 bundle，CSP 无新增面）。

use crate::features::quick_resolution::model::{DisplayInfo, QrError};
use crate::features::quick_resolution::platform::DisplayBackend;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

const OVERLAY_W: f64 = 220.0;
const OVERLAY_H: f64 = 180.0;
const SHOW_MS: u64 = 3000;

/// 为全部显示器弹出编号叠层。
pub fn show_identify_overlays(
    app: &tauri::AppHandle,
    backend: &dyn DisplayBackend,
    displays: &[DisplayInfo],
) -> Result<(), QrError> {
    // 先关旧叠层（连点防护）。
    close_overlays(app);
    let mut created_any = false;
    for d in displays {
        let (mx, my, mw, mh) = match backend.monitor_rect(&d.gdi_name) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("QR 识别叠层：取 {} 矩形失败：{}", d.gdi_name, e);
                continue;
            }
        };
        // 用逻辑坐标（Tauri inner_size/position 是逻辑像素；monitor_rect 是物理像素，
        // 高 DPI 下会错位）。除以 scale_factor 换算。
        let scale = backend.scale_factor_of(&d.gdi_name).unwrap_or(1.0);
        let (lw, lh) = (mw as f64 / scale, mh as f64 / scale);
        let (lx, ly) = (mx as f64 / scale, my as f64 / scale);
        let cx = lx + lw / 2.0 - OVERLAY_W / 2.0;
        let cy = ly + lh / 2.0 - OVERLAY_H / 2.0;
        let label = format!("qr-identify-{}", d.index);
        let url = format!("index.html?view=qr-identify&n={}", d.index);
        match WebviewWindowBuilder::new(app, &label, WebviewUrl::App(url.into()))
            .title("")
            .decorations(false)
            .transparent(false) // 透明在某些 GPU 上渲染异常导致白块
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .inner_size(OVERLAY_W, OVERLAY_H)
            .position(cx, cy)
            .build()
        {
            Ok(win) => {
                let _ = win.set_ignore_cursor_events(true);
                // 深色背景直接由窗口承载，前端只画数字。
                let _ = win.set_background_color(Some(tauri::window::Color(20, 34, 56, 230)));
                created_any = true;
            }
            Err(e) => {
                tracing::warn!("QR 识别叠层：创建 {} 失败：{}", label, e);
            }
        }
    }
    if !created_any {
        return Err(QrError::Io("未能创建任何识别叠层".into()));
    }
    // 到时自动关闭（用 Tauri 运行时而非裸线程，确保退出时窗口句柄仍有效）。
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(SHOW_MS)).await;
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
