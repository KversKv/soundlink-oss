//! 识别叠层（display.md §8.2）：每块屏幕中央显示巨大编号，3 秒后自动关闭。
//!
//! 实现：纯 Win32 GDI 无边框置顶窗口（**不依赖 Tauri webview**，避免 webview 加载
//! 失败导致「白块但无数字」）。数字直接编码进**窗口类名**（`SoundLinkQrId<N>`），
//! 窗口过程从类名解析，彻底绕开 USERDATA 内存管理。

use crate::features::quick_resolution::model::{DisplayInfo, QrError};
use crate::features::quick_resolution::platform::DisplayBackend;
use windows::core::PCWSTR;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

const SHOW_MS: u64 = 3000;
const CLASS_PREFIX: &str = "SoundLinkQrId";

/// 窗口过程：深色底 + 白色大数字（数字从窗口类名解析）。
unsafe extern "system" fn wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            // 从类名 "SoundLinkQrId<N>" 取编号。
            let mut cls = [0u16; 64];
            let n = GetClassNameW(hwnd, &mut cls);
            let text = if n > 0 {
                let name = String::from_utf16_lossy(&cls[..n as usize]);
                name.strip_prefix(CLASS_PREFIX).unwrap_or("?").to_string()
            } else {
                "?".into()
            };
            let mut rc = RECT::default();
            let _ = GetClientRect(hwnd, &mut rc);
            // 深色底（BGR: 20,34,56）。
            let brush = CreateSolidBrush(COLORREF(0x00382214));
            FillRect(hdc, &rc, brush);
            let _ = DeleteObject(brush);
            // 白色大数字居中。
            SetBkMode(hdc, TRANSPARENT);
            SetTextColor(hdc, COLORREF(0x00FFFFFF));
            let font = CreateFontW(
                -96, 0, 0, 0, FW_HEAVY.0 as i32, 0, 0, 0,
                DEFAULT_CHARSET.0 as u32, OUT_DEFAULT_PRECIS.0 as u32, CLIP_DEFAULT_PRECIS.0 as u32,
                CLEARTYPE_QUALITY.0 as u32, DEFAULT_PITCH.0 as u32, windows::core::w!("Segoe UI"),
            );
            let old = SelectObject(hdc, font);
            let mut wide: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
            let _ = DrawTextW(hdc, &mut wide, &mut rc, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
            SelectObject(hdc, old);
            let _ = DeleteObject(font);
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// 为全部显示器弹出编号叠层（GDI 直绘，不依赖 webview）。
pub fn show_identify_overlays(
    _app: &tauri::AppHandle,
    backend: &dyn DisplayBackend,
    displays: &[DisplayInfo],
) -> Result<(), QrError> {
    close_overlays_gdi();
    let mut created = 0usize;
    for d in displays {
        let (mx, my, mw, mh) = match backend.monitor_rect(&d.gdi_name) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("QR 识别叠层：取 {} 矩形失败：{}", d.gdi_name, e);
                continue;
            }
        };
        // GDI 窗口用物理像素。
        let w = 220i32;
        let h = 180i32;
        let x = mx + (mw as i32 - w) / 2;
        let y = my + (mh as i32 - h) / 2;
        // 每窗口独立类名（含编号），窗口过程从类名读数字。
        let class_name = format!("{}{}", CLASS_PREFIX, d.index);
        let class: Vec<u16> = class_name.encode_utf16().chain(Some(0)).collect();
        unsafe {
            let hinst = GetModuleHandleW(None).unwrap_or_default();
            let wc = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: HINSTANCE(hinst.0),
                lpszClassName: PCWSTR(class.as_ptr()),
                hbrBackground: HBRUSH::default(),
                ..Default::default()
            };
            let _ = RegisterClassW(&wc);
            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE,
                PCWSTR(class.as_ptr()),
                PCWSTR(class.as_ptr()),
                WS_POPUP | WS_VISIBLE,
                x,
                y,
                w,
                h,
                HWND::default(),
                HMENU::default(),
                HINSTANCE(hinst.0),
                None,
            );
            match hwnd {
                Ok(hwnd) => {
                    let _ = ShowWindow(hwnd, SW_SHOWNA);
                    let _ = UpdateWindow(hwnd);
                    created += 1;
                }
                Err(e) => {
                    tracing::warn!("QR 识别叠层：CreateWindowExW 失败：{}", e);
                }
            }
        }
    }
    if created == 0 {
        return Err(QrError::Io("未能创建任何识别叠层".into()));
    }
    // 3 秒后统一销毁。
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(SHOW_MS));
        close_overlays_gdi();
    });
    Ok(())
}

/// 关闭全部 GDI 识别叠层（枚举顶层窗口，类名前缀匹配即销毁）。
fn close_overlays_gdi() {
    unsafe {
        unsafe extern "system" fn enum_cb(hwnd: HWND, _lp: LPARAM) -> BOOL {
            let mut cls = [0u16; 64];
            let n = GetClassNameW(hwnd, &mut cls);
            if n > 0 {
                let name = String::from_utf16_lossy(&cls[..n as usize]);
                if name.starts_with(CLASS_PREFIX) {
                    let _ = DestroyWindow(hwnd);
                }
            }
            BOOL(1)
        }
        let _ = EnumWindows(Some(enum_cb), LPARAM(0));
    }
}

/// 供 service 层调用的关闭入口（保持旧签名兼容）。
pub fn close_overlays(_app: &tauri::AppHandle) {
    close_overlays_gdi();
}
