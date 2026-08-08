//! NVAPI 逐步探针：定位哪一步崩溃（用于诊断 display.md M0）。
//! 每步打印后立即 flush，崩溃时最后一条输出即出错点。

use std::io::Write;

fn main() {
    let out = std::io::stdout();
    let mut o = out.lock();
    macro_rules! step { ($s:expr) => {{ writeln!(o, "{}", $s).unwrap(); o.flush().unwrap(); }}; }

    step!("step1: LoadLibrary nvapi64.dll");
    let lib = unsafe {
        windows::Win32::System::LibraryLoader::LoadLibraryW(windows::core::w!("nvapi64.dll"))
    };
    step!(format!("  -> {:?}", lib));
    let lib = match lib { Ok(l) => l, Err(_) => { step!("FAIL: nvapi64.dll 未加载"); return; } };

    step!("step2: GetProcAddress nvapi_QueryInterface");
    let qi = unsafe {
        windows::Win32::System::LibraryLoader::GetProcAddress(
            lib, windows::core::PCSTR(b"nvapi_QueryInterface\0".as_ptr()))
    };
    step!(format!("  -> {:?}", qi));
    let qi: unsafe extern "C" fn(u32) -> *mut std::ffi::c_void = match qi {
        Some(f) => unsafe { std::mem::transmute(f) },
        None => { step!("FAIL: 无 QueryInterface"); return; }
    };

    // NVAPI_Initialize ordinal
    step!("step3: probe NvAPI_Initialize (0x0150E828)");
    let init_p = unsafe { qi(0x0150E828) };
    step!(format!("  -> {:?}", init_p));
    if init_p.is_null() { step!("FAIL: Initialize ordinal 为空"); return; }

    step!("step4: 调用 NvAPI_Initialize");
    let init: unsafe extern "C" fn() -> i32 = unsafe { std::mem::transmute(init_p) };
    let st = unsafe { init() };
    step!(format!("  -> status = {}", st));

    // EnumNvidiaDisplays
    step!("step5: probe EnumNvidiaDisplays (0x9ABDD40D)");
    let enum_p = unsafe { qi(0x9ABDD40D) };
    step!(format!("  -> {:?}", enum_p));
    if enum_p.is_null() { step!("FAIL: EnumDisplays ordinal 为空"); return; }

    step!("step6: 调用 EnumNvidiaDisplays(idx 0..)");
    let enum_fn: unsafe extern "C" fn(u32, *mut *mut std::ffi::c_void) -> i32 =
        unsafe { std::mem::transmute(enum_p) };
    let mut handles = Vec::new();
    for i in 0..4u32 {
        let mut h: *mut std::ffi::c_void = std::ptr::null_mut();
        let s = unsafe { enum_fn(i, &mut h) };
        step!(format!("  idx {} -> status={}, handle={:?}", i, s, h));
        if s != 0 || h.is_null() { break; }
        handles.push(h);
    }
    step!(format!("  共 {} 个句柄", handles.len()));

    // GetDisplayPortInfo
    step!("step7: probe GetDisplayPortInfo (0xC64FF367)");
    let dp_p = unsafe { qi(0xC64FF367) };
    step!(format!("  -> {:?}", dp_p));
    if dp_p.is_null() { step!("SKIP: GetDisplayPortInfo 不存在"); return; }

    step!("step8: 探测 GetDisplayPortInfo 结构体版本（3 参：handle, outputId, info）");
    // 真实签名：NvAPI_DISP_GetDisplayPortInfo(handle, outputId, info)
    let dp_fn: unsafe extern "C" fn(*mut std::ffi::c_void, u32, *mut u8) -> i32 =
        unsafe { std::mem::transmute(dp_p) };
    if let Some(h) = handles.first() {
        // outputId 用 0（第一个输出头）。
        for size in [28usize, 32, 40, 48, 56, 64, 72, 80, 96, 128] {
            let mut buf = [0u8; 256];
            let ver = (size as u32) | (1u32 << 16);
            buf[0..4].copy_from_slice(&ver.to_le_bytes());
            let s = unsafe { dp_fn(*h, 0, buf.as_mut_ptr()) };
            step!(format!("  v1 size={} -> status={}", size, s));
            if s == 0 {
                step!("  *** v1 size={} 命中！dump 前 64 字节：");
                for chunk in buf[..64].chunks(16) {
                    let hex: String = chunk.iter().map(|b| format!("{:02x} ", b)).collect();
                    step!(format!("    {}", hex));
                }
                break;
            }
        }
        for vnum in [2u32, 3] {
            for size in [48usize, 56, 64, 72, 80, 96] {
                let mut buf = [0u8; 256];
                let ver = (size as u32) | (vnum << 16);
                buf[0..4].copy_from_slice(&ver.to_le_bytes());
                let s = unsafe { dp_fn(*h, 0, buf.as_mut_ptr()) };
                step!(format!("  v{} size={} -> status={}", vnum, size, s));
                if s == 0 {
                    step!(format!("  *** v{} size={} 命中！", vnum, size));
                    for chunk in buf[..64].chunks(16) {
                        let hex: String = chunk.iter().map(|b| format!("{:02x} ", b)).collect();
                        step!(format!("    {}", hex));
                    }
                    break;
                }
            }
        }
    }

    step!("step9: NvAPI_Unload");
    let unload_p = unsafe { qi(0xD22BDD7E) };
    if !unload_p.is_null() {
        let unload: unsafe extern "C" fn() -> i32 = unsafe { std::mem::transmute(unload_p) };
        let _ = unsafe { unload() };
    }
    step!("done");
}
