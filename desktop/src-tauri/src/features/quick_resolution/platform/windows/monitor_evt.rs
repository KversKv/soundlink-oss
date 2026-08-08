//! 显示器热插拔/模式变更监听（`WM_DISPLAYCHANGE`）。
//!
//! 独立线程跑隐藏 message-only 窗口，事件通过 Tauri `qr://display-changed`
//! 广播给前端（托盘菜单重建/显示器列表刷新由 service 订阅转发）。

use tauri::{AppHandle, Emitter};
use windows::core::PCWSTR;
use windows::Win32::Foundation::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

const CLASS_NAME: &str = "SoundLinkQrDisplayHook";
/// 简单防抖：两次事件最小间隔（热插拔常连发多条）。
const DEBOUNCE_MS: u64 = 500;

/// 启动监听线程（进程生命周期内常驻）。
pub fn start_display_change_hook(app: AppHandle) {
    std::thread::Builder::new()
        .name("qr-display-hook".into())
        .spawn(move || unsafe {
            run_hook_window(app);
        })
        .map_err(|e| tracing::warn!("QR 显示监听线程启动失败：{}", e))
        .ok();
}

unsafe fn run_hook_window(app: AppHandle) {
    let class: Vec<u16> = CLASS_NAME.encode_utf16().chain(Some(0)).collect();

    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_DISPLAYCHANGE | WM_DEVICECHANGE => {
                let ctx = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut HookCtx;
                if !ctx.is_null() {
                    let ctx = &mut *ctx;
                    let now = std::time::Instant::now();
                    if now.duration_since(ctx.last_emit).as_millis() as u64 >= DEBOUNCE_MS {
                        ctx.last_emit = now;
                        let _ = ctx.app.emit("qr://display-changed", ());
                    }
                }
            }
            _ => {}
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }

    struct HookCtx {
        app: AppHandle,
        last_emit: std::time::Instant,
    }

    let hinstance = match GetModuleHandleW(None) {
        Ok(h) => h,
        Err(e) => {
            tracing::warn!("QR 显示监听：GetModuleHandleW 失败：{}", e);
            return;
        }
    };
    let wc = WNDCLASSW {
        lpfnWndProc: Some(wnd_proc),
        hInstance: HINSTANCE(hinstance.0),
        lpszClassName: PCWSTR(class.as_ptr()),
        ..Default::default()
    };
    if RegisterClassW(&wc) == 0 {
        tracing::warn!("QR 显示监听：RegisterClassW 失败");
        return;
    }
    let ctx = Box::new(HookCtx { app, last_emit: std::time::Instant::now() - std::time::Duration::from_secs(60) });
    // 用无父级的隐藏顶层窗口（而非 HWND_MESSAGE）：广播消息（WM_DEVICECHANGE）
    // 只发顶层窗口；WS_VISIBLE 缺省即隐藏。
    let hwnd = match CreateWindowExW(
        WINDOW_EX_STYLE(0),
        PCWSTR(class.as_ptr()),
        PCWSTR(class.as_ptr()),
        WINDOW_STYLE(0),
        0,
        0,
        0,
        0,
        HWND::default(),
        HMENU::default(),
        HINSTANCE(hinstance.0),
        Some(Box::into_raw(ctx) as *const std::ffi::c_void),
    ) {
        Ok(h) => h,
        Err(e) => {
            tracing::warn!("QR 显示监听：CreateWindowExW 失败：{}", e);
            return;
        }
    };
    let _ = hwnd;
    let mut msg = MSG::default();
    while GetMessageW(&mut msg, None, 0, 0).as_bool() {
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
}
