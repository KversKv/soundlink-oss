//! NVAPI 动态加载层（display.md §7.2）：`libloading` 式手动 `LoadLibrary` +
//! `nvapi_QueryInterface(ordinal)`，绝不静态链接；逐函数 feature probe，
//! 缺失即降级，**任何情况不 panic**（§十八-5）。

pub mod custom;
pub mod ffi;

use crate::features::quick_resolution::model::QrError;
use ffi::*;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::core::PCSTR;
use windows::Win32::Foundation::*;
use windows::Win32::System::LibraryLoader::*;

/// NVAPI 会话（加载 nvapi64.dll + 已 probe 的函数指针集）。
pub struct NvApi {
    lib: HMODULE,
    query: NvapiQueryInterfaceFn,
    pub initialize: Option<NvApiInitializeFn>,
    pub unload: Option<NvApiUnloadFn>,
    pub enum_displays: Option<NvApiEnumDisplaysFn>,
    pub get_dp_info: Option<NvApiGetDisplayPortInfoFn>,
    pub get_timing: Option<NvApiGetTimingFn>,
    pub get_edid: Option<NvApiGetEdidFn>,
    pub get_error_string: Option<NvApiGetErrorMessageFn>,
    pub try_custom_display: Option<NvApiTryCustomDisplayFn>,
    pub save_custom_display: Option<NvApiSaveCustomDisplayFn>,
    pub revert_custom_display: Option<NvApiRevertCustomDisplayTrialFn>,
}

static LOAD_ATTEMPTED: AtomicBool = AtomicBool::new(false);

impl NvApi {
    /// 尝试加载 NVAPI。非 NVIDIA 系统/驱动缺失 → `NvApiUnavailable`。
    pub fn load() -> Result<Self, QrError> {
        LOAD_ATTEMPTED.store(true, Ordering::SeqCst);
        unsafe {
            let lib = LoadLibraryW(windows::core::w!("nvapi64.dll"))
                .map_err(|_| QrError::NvApiUnavailable)?;
            let query_sym = GetProcAddress(lib, PCSTR(b"nvapi_QueryInterface\0".as_ptr()));
            let query = match query_sym {
                Some(f) => std::mem::transmute::<_, NvapiQueryInterfaceFn>(f),
                None => {
                    let _ = FreeLibrary(lib);
                    return Err(QrError::NvApiUnavailable);
                }
            };
            let mut api = Self {
                lib,
                query,
                initialize: None,
                unload: None,
                enum_displays: None,
                get_dp_info: None,
                get_timing: None,
                get_edid: None,
                get_error_string: None,
                try_custom_display: None,
                save_custom_display: None,
                revert_custom_display: None,
            };
            api.probe_all();
            // Initialize 失败 = NVAPI 实际不可用（非 N 卡）。
            let init = api.initialize.ok_or(QrError::NvApiUnavailable)?;
            let status = init();
            if status != 0 {
                tracing::info!("NVAPI Initialize 失败 status={}（可能非 NVIDIA 卡）", status);
                let _ = FreeLibrary(api.lib);
                return Err(QrError::NvApiUnavailable);
            }
            Ok(api)
        }
    }

    /// 逐函数 probe：ordinal → 函数指针，缺失即 None（不 panic）。
    #[allow(clippy::missing_transmute_annotations)]
    fn probe<T>(&self, ordinal: u32) -> Option<T> {
        unsafe {
            let p = (self.query)(ordinal);
            if p.is_null() {
                None
            } else {
                Some(std::mem::transmute_copy(&p))
            }
        }
    }

    fn probe_all(&mut self) {
        self.initialize = self.probe(ordinals::NVAPI_INITIALIZE);
        self.unload = self.probe(ordinals::NVAPI_UNLOAD);
        self.enum_displays = self.probe(ordinals::NVAPI_ENUM_NVIDIA_DISPLAYS);
        self.get_dp_info = self.probe(ordinals::NVAPI_GET_DISPLAY_PORT_INFO);
        self.get_timing = self.probe(ordinals::NVAPI_DISP_GET_TIMING);
        self.get_edid = self.probe(ordinals::NVAPI_DISP_GET_EDID);
        self.get_error_string = self.probe(ordinals::NVAPI_GET_ERROR_MESSAGE);
        self.try_custom_display = self.probe(ordinals::NVAPI_DISP_TRY_CUSTOM_DISPLAY);
        self.save_custom_display = self.probe(ordinals::NVAPI_DISP_SAVE_CUSTOM_DISPLAY);
        self.revert_custom_display = self.probe(ordinals::NVAPI_DISP_REVERT_CUSTOM_DISPLAY);
    }

    /// 探测任意 ordinal 是否被驱动支持（返回非空指针）。
    /// 仅诊断用：nvapi_QueryInterface 对未知 ordinal 返回 NULL。
    pub fn probe_ordinal_present(&self, ordinal: u32) -> bool {
        unsafe { !(self.query)(ordinal).is_null() }
    }

    /// 状态码 → 文本（取不到函数指针时返回数字）。
    pub fn status_text(&self, status: i32) -> String {
        match self.get_error_string {
            Some(f) => unsafe {
                let mut buf = [0i8; 64];
                f(status, buf.as_mut_ptr());
                let c = std::ffi::CStr::from_ptr(buf.as_ptr());
                c.to_string_lossy().into_owned()
            },
            None => format!("status {}", status),
        }
    }
}

impl Drop for NvApi {
    fn drop(&mut self) {
        unsafe {
            if let Some(unload) = self.unload {
                unload();
            }
            let _ = FreeLibrary(self.lib);
        }
    }
}

// NvApi 跨线程传递安全：函数指针 + HMODULE 均为可移动句柄。
unsafe impl Send for NvApi {}
unsafe impl Sync for NvApi {}

/// 是否已探测到 NVIDIA 平台（加载成功即视为 NVIDIA 驱动接口可用）。
pub fn nvidia_present() -> bool {
    NvApi::load().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_never_panics() {
        // 非 N 卡机器上返回 Err，N 卡机器上 Ok——两种情况都不允许 panic。
        // 注意：本机有 NVIDIA 驱动时该测试会真实加载 nvapi64.dll。
        let r = NvApi::load();
        match r {
            Ok(_api) => {
                // 不调用任何函数（结构体布局存疑，调用可能 UB）——仅验证加载不 panic。
            }
            Err(QrError::NvApiUnavailable) => {}
            Err(e) => panic!("意外错误：{}", e),
        }
    }

    #[test]
    fn status_text_fallback() {
        // 不加载 NVAPI，仅验证 ordinal 常量非零（FFI 调用的 UB 风险见 custom.rs 注释）。
        assert_ne!(ffi::ordinals::NVAPI_INITIALIZE, 0);
        assert_ne!(ffi::ordinals::NVAPI_GET_DISPLAY_PORT_INFO, 0);
    }
}
