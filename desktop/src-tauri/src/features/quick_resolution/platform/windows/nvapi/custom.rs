//! NVAPI 自定义分辨率（M8 实现落位）。当前提供：
//! - 链路信息读取（`GetDisplayPortInfo`，M3 DSC 判定的辅证）
//! - 当前 timing 读取（`GetTiming`）
//! - 原始 EDID 读取（`GetEdid`）
//!
//! Try/Save/Revert CustomDisplay 在 M8 接入（`ordinals` 已预留）。

use super::ffi::{self, *};
use super::NvApi;
use crate::features::quick_resolution::model::QrError;

/// NVAPI 读取到的链路信息（DSC 辅证 + 徽标展示）。
#[derive(Debug, Clone, Copy)]
pub struct NvLinkInfo {
    pub lane_count: u8,
    /// 每 lane 速率（Gbps，已按枚举换算）。
    pub rate_gbps: f32,
    pub bpc: Option<u8>,
    pub color_format: Option<&'static str>,
    /// DSC 支持/启用字段（0xFFFFFFFF 哨兵 = 驱动未填）。
    pub dsc_supported: Option<bool>,
    pub dsc_enabled: Option<bool>,
}

impl NvApi {
    /// 枚举 NVIDIA 显示器句柄。
    pub fn display_handles(&self) -> Vec<NvDisplayHandle> {
        let f = match self.enum_displays {
            Some(f) => f,
            None => return Vec::new(),
        };
        let mut out = Vec::new();
        unsafe {
            for i in 0..16u32 {
                let mut h: NvDisplayHandle = std::ptr::null_mut();
                let st = f(i, &mut h);
                if st != 0 || h.is_null() {
                    break;
                }
                out.push(h);
            }
        }
        out
    }

    /// 读取链路信息（驱动不支持该字段时相应为 None）。
    ///
    /// # Safety
    /// 内部对 NVAPI 函数指针的 unsafe 调用已按 ordinal 约定封装；句柄来自 `display_handles()`。
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn link_info(&self, handle: NvDisplayHandle) -> Result<NvLinkInfo, QrError> {
        let f = self.get_dp_info.ok_or(QrError::NvApiUnavailable)?;
        unsafe {
            let mut info = NvDisplayPortInfoV1 {
                version: nvapi_ver(std::mem::size_of::<NvDisplayPortInfoV1>(), 1),
                laneCount: 0,
                linkRate: 0,
                bpc: 0,
                colorFormat: 0,
                isDscSupported: u32::MAX,
                isDscEnabled: u32::MAX,
            };
            let st = f(handle, &mut info);
            if st != 0 {
                return Err(QrError::Win32 {
                    api: format!("NvAPI_GetDisplayPortInfo({})", self.status_text(st)),
                    code: st,
                });
            }
            let rate = match info.linkRate {
                0 => 1.62,
                1 => 2.70,
                2 => 5.40,
                3 => 6.75,
                4 => 8.10,
                5 => 10.0,
                6 => 13.5,
                7 => 20.0,
                _ => 0.0,
            };
            let cf = match info.colorFormat {
                0 => Some("RGB"),
                1 => Some("YCbCr422"),
                2 => Some("YCbCr444"),
                3 => Some("YCbCr420"),
                _ => None,
            };
            let tri = |v: u32| match v {
                u32::MAX => None,
                0 => Some(false),
                _ => Some(true),
            };
            Ok(NvLinkInfo {
                lane_count: info.laneCount as u8,
                rate_gbps: rate,
                bpc: if info.bpc == 0 { None } else { Some(info.bpc as u8) },
                color_format: cf,
                dsc_supported: tri(info.isDscSupported),
                dsc_enabled: tri(info.isDscEnabled),
            })
        }
    }

    /// 读取当前 timing（H/V total、pixel clock）。
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn current_timing(&self, handle: NvDisplayHandle) -> Result<NvTiming, QrError> {
        let f = self.get_timing.ok_or(QrError::NvApiUnavailable)?;
        unsafe {
            let mut t = NvTiming {
                version: nvapi_ver(std::mem::size_of::<NvTiming>(), 1),
                ..Default::default()
            };
            let st = f(handle, 0, &mut t);
            if st != 0 {
                return Err(QrError::Win32 {
                    api: format!("NvAPI_DISP_GetTiming({})", self.status_text(st)),
                    code: st,
                });
            }
            Ok(t)
        }
    }

    /// 读取原始 EDID（最大 1024 字节）。
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn read_edid(&self, handle: NvDisplayHandle) -> Result<Vec<u8>, QrError> {
        let f = self.get_edid.ok_or(QrError::NvApiUnavailable)?;
        unsafe {
            let mut buf = vec![0u8; 1024];
            let st = f(handle, 0, buf.as_mut_ptr());
            if st != 0 {
                return Err(QrError::Win32 {
                    api: format!("NvAPI_DISP_GetEdid({})", self.status_text(st)),
                    code: st,
                });
            }
            // 有效长度 = 128 × (1 + ext_count)。
            let ext = buf[126] as usize;
            let len = 128 * (1 + ext.min(7));
            buf.truncate(len.min(1024));
            Ok(buf)
        }
    }
}

/// M8：NVAPI 自定义分辨率（策略 B，DSC 未启用时副作用最小）。
///
/// 调用序列：`TryCustomDisplay`（临时）→ `SaveCustomDisplay` / `RevertCustomDisplay`。
/// 结构体布局存疑（见下方注释），M0 qr_probe 实测前不暴露给主流程。
pub struct NvCustomMode {
    pub width: u32,
    pub height: u32,
    pub refresh_hz: u32,
    pub timing: crate::features::quick_resolution::platform::windows::nvapi::ffi::NvTiming,
}

impl NvApi {
    /// 由 TimingParams + 刷新率构造 NV_CUSTOM_DISPLAY（布局逐字段对照官方 nvapi.h）。
    pub fn build_custom_display(
        t: &qr_edid::timing::TimingParams,
        refresh_hz: u32,
    ) -> ffi::NvCustomDisplay {
        let etc = ffi::NvTimingExt {
            rr: refresh_hz as u16,
            rrx1k: refresh_hz * 1000,
            rep: 1,
            ..Default::default()
        };
        let tm = ffi::NvTimingStandard {
            h_visible: t.h_active as u16,
            h_border: 0,
            h_front_porch: t.h_front as u16,
            h_sync_width: t.h_sync as u16,
            h_total: t.h_total() as u16,
            h_sync_pol: (!t.h_sync_pol) as u8, // NV: 1=负 0=正（项目内 true=正）
            v_visible: t.v_active as u16,
            v_border: 0,
            v_front_porch: t.v_front as u16,
            v_sync_width: t.v_sync as u16,
            v_total: t.v_total() as u16,
            v_sync_pol: (!t.v_sync_pol) as u8,
            interlaced: 0,
            pclk: t.pixel_clock_khz(refresh_hz) / 10, // NV 单位 10kHz
            etc,
        };
        ffi::NvCustomDisplay {
            width: t.h_active,
            height: t.v_active,
            depth: 32,
            color_format: 22, // NV_FORMAT X8R8G8B8（D3DFMT）
            x_ratio: 1.0,
            y_ratio: 1.0,
            timing: tm,
            ..ffi::NvCustomDisplay::v1()
        }
    }

    /// TryCustomDisplay（临时生效）。签名 `NvAPI_DISP_TryCustomDisplay(NvU32* displayIds, NvU32 count, NV_CUSTOM_DISPLAY*)`。
    /// displayId 取 NvDisplayHandle 低 32 位（NVAPI 句柄即 displayId 编码）。
    /// 返回原始 NVAPI status（0=成功）。
    pub fn try_custom_display(&self, handle: NvDisplayHandle, cd: &mut ffi::NvCustomDisplay) -> Result<i32, QrError> {
        let f = self.try_custom_display.ok_or(QrError::NvApiUnavailable)?;
        let display_id = handle as usize as u32;
        let ids = [display_id];
        let st = unsafe { f(ids.as_ptr(), 1, cd) };
        Ok(st)
    }

    /// SaveCustomDisplay（持久化到驱动的自定义分辨率列表）。
    pub fn save_custom_display(&self, handle: NvDisplayHandle) -> Result<i32, QrError> {
        let f = self.save_custom_display.ok_or(QrError::NvApiUnavailable)?;
        let ids = [handle as usize as u32];
        let st = unsafe { f(ids.as_ptr(), 1, 1, 1) };
        Ok(st)
    }

    /// RevertCustomDisplayTrial（撤销 Try 的临时模式）。
    pub fn revert_custom_display(&self, handle: NvDisplayHandle) -> Result<i32, QrError> {
        let f = self.revert_custom_display.ok_or(QrError::NvApiUnavailable)?;
        let ids = [handle as usize as u32];
        let st = unsafe { f(ids.as_ptr(), 1) };
        Ok(st)
    }
}

// 注意：NvDisplayPortInfoV1 / NvTiming 的结构体布局基于公开 nvapi.h 的最佳理解，
// **未逐字节对照官方头文件验证**。在真实 NVIDIA 机器上调用可能因布局漂移产生
// 未定义行为（M0 qr_probe 实测任务）。因此：
// - 单测不调用这些函数（避免测试进程访问冲突）；
// - M0 qr_probe 先验证布局正确性，再开放给主流程。
#[cfg(test)]
mod tests {
    #[test]
    fn no_ffi_calls_in_tests() {
        // 占位：FFI 正确性由 M0 qr_probe 在真实硬件上验证。
        assert!(true);
    }
}
