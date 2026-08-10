//! NVAPI FFI 类型与函数指针定义（display.md §7.2）。
//!
//! 结构体字段按官方 nvapi.h 布局；版本字段用 `MAKE_NVAPI_VERSION` 编码
//! （`ver = size | (version << 16)`，宏层见各构造点）。

#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(dead_code)]

/// 函数 ordinal（nvapi_interface.h 稳定值，跨驱动版本不变）。
pub mod ordinals {
    pub const NVAPI_INITIALIZE: u32 = 0x0150E828;
    pub const NVAPI_UNLOAD: u32 = 0xD22BDD7E;
    pub const NVAPI_GET_ERROR_MESSAGE: u32 = 0x6C2D048C;
    pub const NVAPI_ENUM_NVIDIA_DISPLAYS: u32 = 0x9ABDD40D;
    pub const NVAPI_GET_DISPLAY_PORT_INFO: u32 = 0xC64FF367;
    pub const NVAPI_DISP_GET_TIMING: u32 = 0x175165E9;
    pub const NVAPI_DISP_GET_EDID: u32 = 0x37D4CC8D;
    // M8 自定义分辨率：
    pub const NVAPI_DISP_TRY_CUSTOM_DISPLAY: u32 = 0x1F7DB630;
    pub const NVAPI_DISP_SAVE_CUSTOM_DISPLAY: u32 = 0x998828C1;
    pub const NVAPI_DISP_REVERT_CUSTOM_DISPLAY: u32 = 0xC40D1268;
}

/// NVAPI 版本编码：`struct_size | (version << 16)`。
#[inline]
pub const fn nvapi_ver(size: usize, version: u32) -> u32 {
    (size as u32) | (version << 16)
}

pub type NvapiQueryInterfaceFn = unsafe extern "C" fn(u32) -> *mut std::ffi::c_void;
pub type NvApiInitializeFn = unsafe extern "C" fn() -> i32;
pub type NvApiUnloadFn = unsafe extern "C" fn() -> i32;
pub type NvApiGetErrorMessageFn = unsafe extern "C" fn(i32, *mut i8) -> i32;

pub type NvDisplayHandle = *mut std::ffi::c_void;

/// NvAPI_EnumNvidiaDisplays: (u32 index, *mut NvDisplayHandle) -> i32
pub type NvApiEnumDisplaysFn = unsafe extern "C" fn(u32, *mut NvDisplayHandle) -> i32;

/// DP 链路速率枚举（NV_DP_LINK_RATE）。
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NvDpLinkRate {
    RBR = 0x0,      // 1.62 Gbps
    HBR = 0x1,      // 2.70
    HBR2 = 0x2,     // 5.40
    HBR2_5 = 0x3,   // 6.75
    HBR3 = 0x4,     // 8.10
    UHBR10 = 0x5,   // 10.0
    UHBR13_5 = 0x6, // 13.5
    UHBR20 = 0x7,   // 20.0
}

/// 色彩格式（NV_COLOR_FORMAT 子集）。
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NvColorFormat {
    Rgb = 0,
    YCbCr422 = 1,
    YCbCr444 = 2,
    YCbCr420 = 3,
    Unknown = 0xFF,
}

/// NV_DISPLAY_PORT_INFO_V1（nvapi.h：v1 含 lane/rate/bpc/colorFormat）。
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NvDisplayPortInfoV1 {
    pub version: u32,
    pub laneCount: u32,
    pub linkRate: u32,     // NvDpLinkRate
    pub bpc: u32,          // bits per color (6/8/10/12/16)
    pub colorFormat: u32,  // NvColorFormat
    pub isDscSupported: u32,
    pub isDscEnabled: u32, // DSC 当前 active 证据字段（feature probe：驱动不填则保持 0xFFFFFFFF 哨兵）
}

pub type NvApiGetDisplayPortInfoFn =
    unsafe extern "C" fn(NvDisplayHandle, *mut NvDisplayPortInfoV1) -> i32;

/// NV_TIMING（NvAPI_DISP_GetTiming，子集：我们只用 total/active/pclk）。
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct NvTiming {
    pub version: u32,
    pub h_active: u16,
    pub h_front: u16,
    pub h_sync: u16,
    pub h_back: u16,
    pub h_total: u16,
    pub v_active: u16,
    pub v_front: u16,
    pub v_sync: u16,
    pub v_back: u16,
    pub v_total: u16,
    pub interlaced: u8,
    pub _pad: [u8; 3],
    pub pclk_khz: u32, // pixel clock in kHz
    pub h_sync_pol: u8,
    pub v_sync_pol: u8,
    pub _pad2: [u8; 6],
}

pub type NvApiGetTimingFn =
    unsafe extern "C" fn(NvDisplayHandle, u32 /*timingTarget, 0=current*/, *mut NvTiming) -> i32;

/// NvAPI_DISP_GetEdid: (handle, u32 offset, *mut u8 buf(EDID 长度上限 1024)) -> i32
pub type NvApiGetEdidFn = unsafe extern "C" fn(NvDisplayHandle, u32, *mut u8) -> i32;

// ---- M8：CustomDisplay（NV_CUSTOM_DISPLAY / NV_TIMING，布局逐字段对照官方 nvapi.h） ----

/// NV_TIMINGEXT（NV_TIMING 尾部内嵌扩展，官方布局，无独立 version）。
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NvTimingExt {
    pub flag: u32,      // 保留（double-scan 等硬件增强），0
    pub rr: u16,        // 逻辑刷新率（整数 Hz）
    pub rrx1k: u32,     // 物理垂直刷新率 ×1000（165Hz = 165000）
    pub aspect: u32,    // Hi:水平宽高比 Low:垂直宽高比（0 = 驱动自算）
    pub rep: u16,       // 像素重复因子（1 = 无重复）
    pub status: u32,    // timing 标准（0）
    pub name: [u8; 40], // timing 名（可全 0）
}
impl Default for NvTimingExt {
    fn default() -> Self {
        Self { flag: 0, rr: 0, rrx1k: 0, aspect: 0, rep: 1, status: 0, name: [0; 40] }
    }
}

/// NV_TIMING（官方布局；**无 version 字段**，pclk 单位 10kHz，border=0）。
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct NvTimingStandard {
    pub h_visible: u16,
    pub h_border: u16,
    pub h_front_porch: u16,
    pub h_sync_width: u16,
    pub h_total: u16,
    pub h_sync_pol: u8, // 1=负 0=正
    pub v_visible: u16,
    pub v_border: u16,
    pub v_front_porch: u16,
    pub v_sync_width: u16,
    pub v_total: u16,
    pub v_sync_pol: u8, // 1=负 0=正
    pub interlaced: u16, // 0=逐行
    pub pclk: u32,       // 10kHz 单位
    pub etc: NvTimingExt,
}

/// NV_VIEWPORTF（multimon 分区，应为 (0,0,1.0,1.0)）。
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NvViewportF {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}
impl Default for NvViewportF {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0, w: 1.0, h: 1.0 }
    }
}

/// NV_CUSTOM_DISPLAY（Try/SaveCustomDisplay 输入，官方布局）。
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct NvCustomDisplay {
    pub version: u32,
    pub width: u32,
    pub height: u32,
    pub depth: u32,        // 色深 bpp；0 = 全部 8/16/32
    pub color_format: u32, // NV_FORMAT（D3DFORMAT；22 = X8R8G8B8）
    pub src_partition: NvViewportF,
    pub x_ratio: f32,
    pub y_ratio: f32,
    pub timing: NvTimingStandard,
    pub hw_mode_set_only: u32, // bitfield:1；0 = 含 OS 更新的完整 modeset
}

impl NvCustomDisplay {
    pub fn v1() -> Self {
        Self {
            version: nvapi_ver(std::mem::size_of::<NvCustomDisplay>(), 1),
            ..Default::default()
        }
    }
}

pub type NvApiTryCustomDisplayFn =
    unsafe extern "C" fn(*const u32 /*displayIds*/, u32 /*count*/, *mut NvCustomDisplay) -> i32;
pub type NvApiSaveCustomDisplayFn =
    unsafe extern "C" fn(*const u32 /*displayIds*/, u32 /*count*/, u32 /*isThisOutputIdOnly*/, u32 /*isThisMonitorIdOnly*/) -> i32;
pub type NvApiRevertCustomDisplayTrialFn = unsafe extern "C" fn(*const u32 /*displayIds*/, u32 /*count*/) -> i32;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_encoding() {
        // ver1：size | 1<<16
        let v = nvapi_ver(100, 1);
        assert_eq!(v & 0xFFFF, 100);
        assert_eq!(v >> 16, 1);
    }

    #[test]
    fn struct_sizes_stable() {
        // 布局漂移防护（字段改动会被此处暴露）。
        // NvDisplayPortInfoV1 = 7×u32 = 28 字节。
        assert_eq!(std::mem::size_of::<NvDisplayPortInfoV1>(), 28);
        assert!(std::mem::size_of::<NvTiming>() >= 36);
    }
}
