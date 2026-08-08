//! 识别叠层（display.md §8.2）：每块屏幕中央显示巨大编号，3 秒后自动关闭。
//!
//! 实现：纯 Win32 GDI 无边框置顶窗口（**不依赖 Tauri webview**，避免 webview 加载
//! 失败导致「白块但无数字」）。数字用 GDI `DrawTextW` 直接画，零前端不确定性。

use crate::features::quick_resolution::model::{DisplayInfo, QrError};
use crate::features::quick_resolution::platform::DisplayBackend;
use windows::core::PCWSTR;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

const SHOW_MS: u64 = 3000;
const CLASS_NAME: &str = "SoundLinkQrIdentify";

/// 窗口过程：画深色底 + 白色大数字。
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
            // 读编号（存在 GWLP_USERDATA 里，ASCII 字符串指针）。
            let n_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const u8;
            let text = if !n_ptr.is_null() {
                let mut len = 0usize;
                while *n_ptr.add(len) != 0 {
                    len += 1;
                }
                String::from_utf8_lossy(std::slice::from_raw_parts(n_ptr, len)).into_owned()
            } else {
                "?".into()
            };
            let mut rc = RECT::default();
            let _ = GetClientRect(hwnd, &mut rc);
            // 深色底。
            let brush = CreateSolidBrush(COLORREF(0x00382214)); // BGR: 20,34,56
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
        WM_DESTROY => {
            // 释放 USERDATA 里的字符串。
            let p = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut u8;
            if !p.is_null() {
                let _ = Box::from_raw(p);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            }
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
        // GDI 窗口用物理像素（SetWindowPos 是物理坐标）。
        let w = 220i32;
        let h = 180i32;
        let x = mx + (mw as i32 - w) / 2;
        let y = my + (mh as i32 - h) / 2;
        let text = d.index.to_string();
        let text_box = Box::into_raw(text.into_bytes().into_boxed_slice()) as *mut u8;
        unsafe {
            let hinst = GetModuleHandleW(None).unwrap_or_default();
            let class: Vec<u16> = CLASS_NAME.encode_utf16().chain(Some(0)).collect();
            // 注册类（重复注册失败无碍）。
            let wc = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: hinst.into(),
                lpszClassName: windows::core::PCWSTR(class.as_ptr()),
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
                Ok(h) => {
                    SetWindowLongPtrW(h, GWLP_USERDATA, text_box as isize);
                    let _ = ShowWindow(h, SW_SHOWNA);
                    let _ = UpdateWindow(h);
                    created += 1;
                }
                Err(e) => {
                    let _ = Box::from_raw(text_box);
                    tracing::warn!("QR 识别叠层：CreateWindowExW 失败：{}", e);
                }
            }
        }
    }
    if created == 0 {
        return Err(QrError::Io("未能创建任何识别叠层".into()));
    }
    // 3 秒后统一销毁（独立线程 + FindWindow 枚举我们的类名）。
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(SHOW_MS));
        close_overlays_gdi();
    });
    Ok(())
}

/// 关闭全部 GDI 识别叠层（枚举线程窗口不可行，用广播 WM_CLOSE 给我们的类）。
fn close_overlays_gdi() {
    unsafe {
        // EnumWindows 找我们的类名，逐个 DestroyWindow。
        unsafe extern "system" fn enum_cb(hwnd: HWND, _lp: LPARAM) -> BOOL {
            let mut cls = [0u16; 64];
            let n = GetClassNameW(hwnd, &mut cls);
            if n > 0 {
                let name = String::from_utf16_lossy(&cls[..n as usize]);
                if name == CLASS_NAME {
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
