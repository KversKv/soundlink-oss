//! GDI 显示模式操作：枚举模式 / 快切 / 当前模式 / 主屏判定 / 屏幕矩形。
//!
//! 设计约束（display.md §7.1）：CCD 只用于枚举/标识/快照回滚，
//! 模式切换只走 `ChangeDisplaySettingsEx`（避免 `SetDisplayConfig` 的拓扑副作用）。

use crate::features::quick_resolution::model::{QrError, SystemMode};
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::Threading::*;
use windows::Win32::UI::WindowsAndMessaging::*;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// 枚举系统已注册模式（去重：同 宽×高×刷新×色深 只留一条）。
pub fn enum_modes(gdi_name: &str) -> Result<Vec<SystemMode>, QrError> {
    unsafe {
        let w = wide(gdi_name);
        let mut out: Vec<SystemMode> = Vec::new();
        let mut i = 0u32;
        loop {
            let mut dm = DEVMODEW::default();
            dm.dmSize = std::mem::size_of::<DEVMODEW>() as u16;
            let ok = EnumDisplaySettingsExW(
                PCWSTR(w.as_ptr()),
                ENUM_DISPLAY_SETTINGS_MODE(i),
                &mut dm,
                ENUM_DISPLAY_SETTINGS_FLAGS(0),
            );
            if !ok.as_bool() {
                break;
            }
            let m = SystemMode {
                width: dm.dmPelsWidth,
                height: dm.dmPelsHeight,
                refresh_hz: dm.dmDisplayFrequency,
                bits_per_pel: dm.dmBitsPerPel,
            };
            if !out.contains(&m) {
                out.push(m);
            }
            i += 1;
            if i > 4096 {
                break; // 防御异常驱动
            }
        }
        Ok(out)
    }
}

/// 当前模式（ENUM_CURRENT_SETTINGS）。
pub fn current_mode(gdi_name: &str) -> Result<SystemMode, QrError> {
    unsafe {
        let w = wide(gdi_name);
        let mut dm = DEVMODEW::default();
        dm.dmSize = std::mem::size_of::<DEVMODEW>() as u16;
        let ok = EnumDisplaySettingsExW(
            PCWSTR(w.as_ptr()),
            ENUM_CURRENT_SETTINGS,
            &mut dm,
            ENUM_DISPLAY_SETTINGS_FLAGS(0),
        );
        if !ok.as_bool() {
            return Err(QrError::Win32 { api: "EnumDisplaySettingsEx(CURRENT)".into(), code: -1 });
        }
        Ok(SystemMode {
            width: dm.dmPelsWidth,
            height: dm.dmPelsHeight,
            refresh_hz: dm.dmDisplayFrequency,
            bits_per_pel: dm.dmBitsPerPel,
        })
    }
}

/// 主屏判定：当前位置 (0,0)。
pub fn is_primary(gdi_name: &str) -> bool {
    unsafe {
        let w = wide(gdi_name);
        let mut dm = DEVMODEW::default();
        dm.dmSize = std::mem::size_of::<DEVMODEW>() as u16;
        let ok = EnumDisplaySettingsExW(
            PCWSTR(w.as_ptr()),
            ENUM_CURRENT_SETTINGS,
            &mut dm,
            ENUM_DISPLAY_SETTINGS_FLAGS(0),
        );
        if !ok.as_bool() {
            return false;
        }
        let pos = dm.Anonymous1.Anonymous2.dmPosition;
        pos.x == 0 && pos.y == 0
    }
}

/// 快切（display.md §7.1）：精确匹配已注册模式 → CDS_TEST → CDS_UPDATEREGISTRY。
pub fn apply(gdi_name: &str, mode: &SystemMode) -> Result<(), QrError> {
    // 1) 精确匹配系统已注册模式，避免 Windows 取整/回退。
    let matched = enum_modes(gdi_name)?
        .into_iter()
        .find(|d| d.width == mode.width && d.height == mode.height && d.refresh_hz == mode.refresh_hz)
        .ok_or(QrError::ModeNotRegistered)?;

    unsafe {
        let w = wide(gdi_name);
        let mut dm = DEVMODEW::default();
        dm.dmSize = std::mem::size_of::<DEVMODEW>() as u16;
        // 取注册表完整模式定义（含 display flags），再覆盖关键字段。
        let ok = EnumDisplaySettingsExW(
            PCWSTR(w.as_ptr()),
            ENUM_REGISTRY_SETTINGS,
            &mut dm,
            ENUM_DISPLAY_SETTINGS_FLAGS(0),
        );
        if !ok.as_bool() {
            return Err(QrError::Win32 { api: "EnumDisplaySettingsEx(REGISTRY)".into(), code: -1 });
        }
        dm.dmPelsWidth = matched.width;
        dm.dmPelsHeight = matched.height;
        dm.dmDisplayFrequency = matched.refresh_hz;
        dm.dmBitsPerPel = matched.bits_per_pel;
        dm.dmFields = DM_PELSWIDTH | DM_PELSHEIGHT | DM_DISPLAYFREQUENCY | DM_BITSPERPEL;

        let t = ChangeDisplaySettingsExW(
            PCWSTR(w.as_ptr()),
            Some(&dm),
            HWND::default(),
            CDS_TEST,
            None,
        );
        if t != DISP_CHANGE_SUCCESSFUL {
            return Err(QrError::Win32 { api: "CDS_TEST".into(), code: t.0 });
        }
        let r = ChangeDisplaySettingsExW(
            PCWSTR(w.as_ptr()),
            Some(&dm),
            HWND::default(),
            CDS_UPDATEREGISTRY | CDS_GLOBAL,
            None,
        );
        if r != DISP_CHANGE_SUCCESSFUL {
            return Err(QrError::Win32 { api: "CDS_APPLY".into(), code: r.0 });
        }
    }
    Ok(())
}

/// 显示器屏幕矩形（识别叠层定位）。
pub fn monitor_rect(gdi_name: &str) -> Result<(i32, i32, u32, u32), QrError> {
    struct Ctx {
        target: String,
        found: Option<(i32, i32, u32, u32)>,
    }
    unsafe extern "system" fn cb(
        hmon: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        lparam: LPARAM,
    ) -> BOOL {
        let ctx = &mut *(lparam.0 as *mut Ctx);
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(hmon, &mut info as *mut _ as *mut MONITORINFO).as_bool() {
            let name = super::ccd::wide_to_string(&info.szDevice);
            if name == ctx.target {
                let r = info.monitorInfo.rcMonitor;
                ctx.found = Some((
                    r.left,
                    r.top,
                    (r.right - r.left) as u32,
                    (r.bottom - r.top) as u32,
                ));
                return BOOL(0); // 停止枚举
            }
        }
        BOOL(1)
    }
    let mut ctx = Ctx { target: gdi_name.to_string(), found: None };
    unsafe {
        let _ = EnumDisplayMonitors(
            HDC::default(),
            None,
            Some(cb),
            LPARAM(&mut ctx as *mut Ctx as isize),
        );
    }
    ctx.found
        .ok_or_else(|| QrError::DisplayNotFound(gdi_name.to_string()))
}

/// 全屏独占启发式检测（预置前置守卫，display.md §7.3 step 0）。
///
/// 规则：前台窗口 == 所在显示器完整矩形，且无标题栏/边框样式，
/// 且不属于本进程 → 视为疑似全屏独占，返回进程映像名。
pub fn fullscreen_exclusive_process() -> Option<String> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == std::process::id() || pid == 0 {
            return None;
        }
        // 排除 shell（桌面/任务栏本身是全屏顶层窗）。
        let mut cls = [0u16; 64];
        let n = GetClassNameW(hwnd, &mut cls);
        let cls_name = if n > 0 { String::from_utf16_lossy(&cls[..n as usize]) } else { String::new() };
        if matches!(cls_name.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd") {
            return None;
        }
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return None;
        }
        let w = rect.right - rect.left;
        let h = rect.bottom - rect.top;
        if w <= 0 || h <= 0 {
            return None;
        }
        // 与所在显示器比较。
        let hmon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONULL);
        if hmon.0.is_null() {
            return None;
        }
        let mut info = MONITORINFO::default();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if !GetMonitorInfoW(hmon, &mut info).as_bool() {
            return None;
        }
        let mr = info.rcMonitor;
        let covers = rect.left <= mr.left
            && rect.top <= mr.top
            && rect.right >= mr.right
            && rect.bottom >= mr.bottom;
        if !covers {
            return None;
        }
        // 样式：无 caption / 无边框 → 疑似独占/无边框全屏。
        let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
        let has_caption = style & WS_CAPTION.0 != 0;
        let has_border = style & WS_THICKFRAME.0 != 0;
        if has_caption || has_border {
            return None;
        }
        // 取进程名。
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 260];
        let mut len = buf.len() as u32;
        let name = if QueryFullProcessImageNameW(handle, PROCESS_NAME_FORMAT(0), PWSTR(buf.as_mut_ptr()), &mut len).is_ok() {
            let full = String::from_utf16_lossy(&buf[..len as usize]);
            let _ = CloseHandle(handle);
            full.rsplit('\\').next().unwrap_or(&full).to_string()
        } else {
            let _ = CloseHandle(handle);
            format!("pid {}", pid)
        };
        Some(name)
    }
}
