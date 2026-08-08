//! 显示链路带宽计算（display.md §6.1 公式的产品化）。
//!
//! 纯函数、零 Windows 依赖，可完整单测。覆盖：
//! - 像素时钟：`f_pixel = H_total × V_total × f_refresh`
//! - 未压缩净带宽：`B_req = f_pixel × bpp_eff`
//! - DP 链路可用净带宽：8b/10b（HBR/HBR2/HBR3）与 128b/132b（UHBR，含 FEC 折损）
//! - HDMI 2.1 FRL：16b/18b 编码
//! - DSC 必要性判定与压缩后可行性校验

use serde::{Deserialize, Serialize};

/// DSC 判定的带宽余量：需求 > 可用 × 0.98 即认为 DSC 必然启用。
pub const DSC_MARGIN: f32 = 0.98;
/// UHBR 链路的 FEC 折损系数（128b/132b 之外的额外协议开销）。
pub const UHBR_FEC_ETA: f32 = 0.98;
/// DSC 目标 bpp 合理区间（VESA DSC 典型 8–12 bpp）。
pub const DSC_BPP_MIN: f32 = 8.0;
pub const DSC_BPP_MAX: f32 = 12.0;

/// 色彩格式（决定有效 bpp）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorFormat {
    Rgb,
    YCbCr444,
    YCbCr422,
    YCbCr420,
}

impl ColorFormat {
    /// 每像素有效比特数（display.md §6.1 定义）。
    pub fn bpp_eff(self, bpc: u8) -> f32 {
        let bpc = bpc as f32;
        match self {
            ColorFormat::Rgb | ColorFormat::YCbCr444 => 3.0 * bpc,
            ColorFormat::YCbCr422 => 2.0 * bpc,
            ColorFormat::YCbCr420 => 1.5 * bpc,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ColorFormat::Rgb => "RGB",
            ColorFormat::YCbCr444 => "YCbCr 4:4:4",
            ColorFormat::YCbCr422 => "YCbCr 4:2:2",
            ColorFormat::YCbCr420 => "YCbCr 4:2:0",
        }
    }
}

/// 链路编码方式（决定净荷效率）。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Coding {
    /// DP HBR 系列：8b/10b。
    EightBTenB,
    /// DP UHBR：128b/132b，另有 FEC 折损。
    OneTwentyEightBOneThirtyTwoB,
    /// HDMI 2.1 FRL：16b/18b。
    FrlSixteenBEighteenB,
}

impl Coding {
    pub fn efficiency(self) -> f32 {
        match self {
            Coding::EightBTenB => 8.0 / 10.0,
            Coding::OneTwentyEightBOneThirtyTwoB => 128.0 / 132.0 * UHBR_FEC_ETA,
            Coding::FrlSixteenBEighteenB => 16.0 / 18.0,
        }
    }
}

/// 物理链路规格。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LinkSpec {
    pub lanes: u8,
    /// 每 lane 符号率（Gbps）。
    pub rate_per_lane_gbps: f32,
    pub coding: Coding,
    /// 展示名（如 "DP2.1 UHBR13.5 ×4"）。
    pub label: &'static str,
}

impl LinkSpec {
    pub fn dp_hbr(lanes: u8) -> Self {
        Self { lanes, rate_per_lane_gbps: 2.7, coding: Coding::EightBTenB, label: "DP HBR" }
    }
    pub fn dp_hbr2(lanes: u8) -> Self {
        Self { lanes, rate_per_lane_gbps: 5.4, coding: Coding::EightBTenB, label: "DP1.2 HBR2" }
    }
    pub fn dp_hbr3(lanes: u8) -> Self {
        Self { lanes, rate_per_lane_gbps: 8.1, coding: Coding::EightBTenB, label: "DP1.4 HBR3" }
    }
    pub fn dp_uhbr10(lanes: u8) -> Self {
        Self {
            lanes,
            rate_per_lane_gbps: 10.0,
            coding: Coding::OneTwentyEightBOneThirtyTwoB,
            label: "DP2.1 UHBR10",
        }
    }
    pub fn dp_uhbr13_5(lanes: u8) -> Self {
        Self {
            lanes,
            rate_per_lane_gbps: 13.5,
            coding: Coding::OneTwentyEightBOneThirtyTwoB,
            label: "DP2.1 UHBR13.5",
        }
    }
    pub fn dp_uhbr20(lanes: u8) -> Self {
        Self {
            lanes,
            rate_per_lane_gbps: 20.0,
            coding: Coding::OneTwentyEightBOneThirtyTwoB,
            label: "DP2.1 UHBR20",
        }
    }
    /// HDMI 2.1 FRL（rate_gbps ∈ {3,6,8,10,12}，4 lanes）。
    pub fn hdmi_frl(rate_gbps: f32) -> Self {
        Self { lanes: 4, rate_per_lane_gbps: rate_gbps, coding: Coding::FrlSixteenBEighteenB, label: "HDMI2.1 FRL" }
    }
    /// HDMI 2.0 TMDS（6 Gbps × 3 数据 lane，8b/10b）。
    pub fn hdmi_tmds() -> Self {
        Self { lanes: 3, rate_per_lane_gbps: 6.0, coding: Coding::EightBTenB, label: "HDMI2.0 TMDS" }
    }

    /// 链路可用净带宽（Gbps）。
    pub fn available_gbps(&self) -> f32 {
        self.lanes as f32 * self.rate_per_lane_gbps * self.coding.efficiency()
    }
}

/// 一套完整时序参数（active + blanking）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Timing {
    pub h_active: u32,
    pub v_active: u32,
    pub h_total: u32,
    pub v_total: u32,
    pub refresh_hz: u32,
}

impl Timing {
    /// 像素时钟（kHz）。
    pub fn pixel_clock_khz(&self) -> u64 {
        // H_total × V_total × Hz 得像素/秒；/1000 得 kHz。
        self.h_total as u64 * self.v_total as u64 * self.refresh_hz as u64 / 1000
    }

    /// 未压缩所需净带宽（Gbps）。
    pub fn required_gbps(&self, bpc: u8, format: ColorFormat) -> f32 {
        let pix_per_sec = self.pixel_clock_khz() as f32 * 1000.0;
        pix_per_sec * format.bpp_eff(bpc) / 1e9
    }

    /// DSC 压缩后所需净带宽（Gbps）。`dsc_bpp` ∈ [8, 12]。
    pub fn required_gbps_dsc(&self, dsc_bpp: f32) -> f32 {
        let pix_per_sec = self.pixel_clock_khz() as f32 * 1000.0;
        pix_per_sec * dsc_bpp / 1e9
    }
}

/// DSC 三路交叉判定中的「带宽推算」结论。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum DscByBandwidth {
    /// 未压缩需求 > 可用 × 0.98 → DSC 必然处于启用状态。
    CertainlyActive,
    /// 未压缩可行。
    NotNeeded,
}

/// 带宽推算：当前时序在给定链路上是否必然依赖 DSC。
pub fn dsc_required(t: &Timing, bpc: u8, format: ColorFormat, link: &LinkSpec) -> DscByBandwidth {
    if t.required_gbps(bpc, format) > link.available_gbps() * DSC_MARGIN {
        DscByBandwidth::CertainlyActive
    } else {
        DscByBandwidth::NotNeeded
    }
}

/// 目标模式可行性评估（编辑器预检用）。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Feasibility {
    pub pixel_clock_khz: u64,
    pub required_uncompressed_gbps: f32,
    pub available_gbps: f32,
    /// 未压缩是否可行。
    pub uncompressed_ok: bool,
    /// DSC 启用时（按 dsc_bpp）所需带宽。
    pub required_dsc_gbps: Option<f32>,
    /// DSC 压缩后是否可行（None = 未评估 DSC 路径）。
    pub dsc_ok: Option<bool>,
}

/// 预检目标模式。
///
/// - `dsc_available`：显示器支持 DSC（EDID 解析结论）或用户强制视为启用；
/// - `dsc_bpp`：压缩目标（None 时按 10bpp 评估）。
pub fn check_feasibility(
    t: &Timing,
    bpc: u8,
    format: ColorFormat,
    link: &LinkSpec,
    dsc_available: bool,
    dsc_bpp: Option<f32>,
) -> Feasibility {
    let available = link.available_gbps();
    let required = t.required_gbps(bpc, format);
    let uncompressed_ok = required <= available * DSC_MARGIN;
    let (required_dsc, dsc_ok) = if dsc_available && !uncompressed_ok {
        let bpp = dsc_bpp.unwrap_or(10.0).clamp(DSC_BPP_MIN, DSC_BPP_MAX);
        let req = t.required_gbps_dsc(bpp);
        (Some(req), Some(req <= available * DSC_MARGIN))
    } else {
        (None, None)
    };
    Feasibility {
        pixel_clock_khz: t.pixel_clock_khz(),
        required_uncompressed_gbps: required,
        available_gbps: available,
        uncompressed_ok,
        required_dsc_gbps: required_dsc,
        dsc_ok,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 文档 §6.1 示例：1920×1440@480Hz（H≈2000, V≈1471）。
    fn mode_1920x1440_480() -> Timing {
        Timing { h_active: 1920, v_active: 1440, h_total: 2000, v_total: 1471, refresh_hz: 480 }
    }

    #[test]
    fn pixel_clock_matches_doc_example() {
        let t = mode_1920x1440_480();
        // 2000×1471×480 = 1,412,160,000 px/s ≈ 1.412 GPix/s。
        assert_eq!(t.pixel_clock_khz(), 1_412_160);
    }

    #[test]
    fn required_bandwidth_10bpc_rgb() {
        let t = mode_1920x1440_480();
        // 1.41216e9 × 30 / 1e9 ≈ 42.4 Gbps。
        let req = t.required_gbps(10, ColorFormat::Rgb);
        assert!((req - 42.4).abs() < 0.1, "req={req}");
    }

    #[test]
    fn uhbr13_5_x4_available() {
        let link = LinkSpec::dp_uhbr13_5(4);
        // 4 × 13.5 × 128/132 × 0.98 ≈ 51.35 Gbps（文档未计 FEC 为 52.4）。
        let avail = link.available_gbps();
        assert!((avail - 51.35).abs() < 0.1, "avail={avail}");
    }

    #[test]
    fn hbr3_x4_available() {
        let link = LinkSpec::dp_hbr3(4);
        // 4 × 8.1 × 0.8 = 25.92 Gbps。
        assert!((link.available_gbps() - 25.92).abs() < 0.01);
    }

    #[test]
    fn hdmi_frl12_available() {
        let link = LinkSpec::hdmi_frl(12.0);
        // 4 × 12 × 16/18 ≈ 42.67 Gbps。
        assert!((link.available_gbps() - 42.67).abs() < 0.01);
    }

    #[test]
    fn dsc_required_on_uhbr10_but_not_uhbr13_5() {
        let t = mode_1920x1440_480();
        // UHBR10 ×4：4×10×128/132×0.98 ≈ 38.0 Gbps < 42.4 → 必然 DSC。
        assert_eq!(
            dsc_required(&t, 10, ColorFormat::Rgb, &LinkSpec::dp_uhbr10(4)),
            DscByBandwidth::CertainlyActive
        );
        // UHBR13.5 ×4 ≈ 51.35 Gbps > 42.4 → 未压缩可行。
        assert_eq!(
            dsc_required(&t, 10, ColorFormat::Rgb, &LinkSpec::dp_uhbr13_5(4)),
            DscByBandwidth::NotNeeded
        );
    }

    #[test]
    fn ycbcr420_quarters_bandwidth() {
        let t = Timing { h_active: 3840, v_active: 2160, h_total: 4000, v_total: 2200, refresh_hz: 60 };
        let rgb = t.required_gbps(8, ColorFormat::Rgb);
        let y420 = t.required_gbps(8, ColorFormat::YCbCr420);
        assert!((rgb / y420 - 2.0).abs() < 0.001, "rgb={rgb} y420={y420}");
    }

    #[test]
    fn feasibility_dsc_path() {
        let t = mode_1920x1440_480();
        let f = check_feasibility(&t, 10, ColorFormat::Rgb, &LinkSpec::dp_uhbr10(4), true, Some(10.0));
        assert!(!f.uncompressed_ok);
        // DSC 10bpp：1.41216e9 × 10 / 1e9 ≈ 14.1 Gbps，可行。
        let req_dsc = f.required_dsc_gbps.unwrap();
        assert!((req_dsc - 14.1).abs() < 0.1);
        assert_eq!(f.dsc_ok, Some(true));
    }

    #[test]
    fn feasibility_no_dsc_when_uncompressed_ok() {
        let t = Timing { h_active: 1920, v_active: 1080, h_total: 2000, v_total: 1111, refresh_hz: 60 };
        let f = check_feasibility(&t, 8, ColorFormat::Rgb, &LinkSpec::dp_hbr2(4), true, None);
        assert!(f.uncompressed_ok);
        assert!(f.dsc_ok.is_none());
    }

    #[test]
    fn feasibility_infeasible_without_dsc_support() {
        let t = mode_1920x1440_480();
        let f = check_feasibility(&t, 10, ColorFormat::Rgb, &LinkSpec::dp_hbr3(4), false, None);
        assert!(!f.uncompressed_ok);
        assert_eq!(f.dsc_ok, None);
    }

    #[test]
    fn color_format_labels() {
        assert_eq!(ColorFormat::Rgb.label(), "RGB");
        assert_eq!(ColorFormat::YCbCr422.label(), "YCbCr 4:2:2");
    }
}
