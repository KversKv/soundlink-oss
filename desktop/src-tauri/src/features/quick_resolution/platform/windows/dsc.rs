//! DSC 三路交叉判定（display.md §6）。
//!
//! ① 带宽推算（主判据，`qr-bandwidth`）
//! ② NVAPI 链路信息（辅证：DP info 的 DSC 字段 + 当前 timing 实测像素时钟）
//! ③ EDID/DisplayID 能力解析（显示器**支持** DSC 与否；支持 ≠ 当前启用）

use super::nvapi::{custom::NvLinkInfo, NvApi};
use crate::features::quick_resolution::model::{DscState, QrError};
use qr_bandwidth::{ColorFormat, DscByBandwidth, LinkSpec};

/// 单块显示器的 DSC 判定结果（含中间量，诊断抽屉展示）。
#[derive(Debug, Clone)]
pub struct DscReport {
    pub state: DscState,
    /// 当前模式的未压缩需求（Gbps）。
    pub required_gbps: Option<f32>,
    /// 链路可用净带宽（Gbps）。
    pub available_gbps: Option<f32>,
    /// 规范化链路标签（如 "DP2.1 UHBR13.5 ×4"）。
    pub link_label: Option<String>,
    pub bpc: Option<u8>,
    pub color_format: Option<String>,
    /// 判定依据（诊断展示）。
    pub basis: Vec<String>,
}

/// 判定某显示器当前 DSC 状态。
///
/// 输入：当前模式（宽/高/刷新）、NVAPI 读到的链路信息（可能 None）、EDID 解析结论。
pub fn detect(
    current: Option<(u32, u32, u32)>,
    nv_link: Option<&NvLinkInfo>,
    edid_dsc_supported: Option<bool>,
    forced: Option<bool>,
) -> DscReport {
    // 用户手动覆盖优先（§6.2）。
    if let Some(on) = forced {
        return DscReport {
            state: DscState::ForcedByUser { on },
            required_gbps: None,
            available_gbps: None,
            link_label: None,
            bpc: None,
            color_format: None,
            basis: vec!["用户手动覆盖".into()],
        };
    }

    let mut basis = Vec::new();
    let mut link_label = None;
    let mut available = None;
    let mut required = None;
    let mut bpc = None;
    let mut cf_str = None;

    // ② NVAPI 辅证
    if let Some(link) = nv_link {
        if let Some(enabled) = link.dsc_enabled {
            basis.push("NVAPI DP info DSC 字段".into());
            link_label = Some(label_of(link));
            available = Some(link_available_gbps(link));
            bpc = link.bpc;
            cf_str = link.color_format.map(|s| s.to_string());
            return DscReport {
                state: if enabled { DscState::Active } else { DscState::Inactive },
                required_gbps: None,
                available_gbps: available,
                link_label,
                bpc,
                color_format: cf_str,
                basis,
            };
        }
        link_label = Some(label_of(link));
        available = Some(link_available_gbps(link));
        bpc = link.bpc;
        cf_str = link.color_format.map(|s| s.to_string());
        basis.push("NVAPI 链路信息".into());
    }

    // ① 带宽推算（主判据）
    if let (Some((w, h, hz)), Some(link)) = (current, nv_link) {
        let t = qr_bandwidth::Timing {
            h_active: w,
            v_active: h,
            // 无精确 total 时用 +5% 消隐估算（保守方向：略高估需求）。
            h_total: (w as f32 * 1.05) as u32,
            v_total: (h as f32 * 1.02) as u32,
            refresh_hz: hz,
        };
        let bpc_v = link.bpc.unwrap_or(8);
        let cf = parse_cf(link.color_format);
        let spec = link_spec_of(link);
        required = Some(t.required_gbps(bpc_v, cf));
        basis.push("带宽推算".into());
        match qr_bandwidth::dsc_required(&t, bpc_v, cf, &spec) {
            DscByBandwidth::CertainlyActive => {
                basis.push(format!(
                    "未压缩需 {:.1} > 可用 {:.1}×0.98",
                    required.unwrap_or(0.0),
                    spec.available_gbps()
                ));
                return DscReport {
                    state: DscState::LikelyActive { confidence: 0.9, basis: basis.clone() },
                    required_gbps: required,
                    available_gbps: Some(spec.available_gbps()),
                    link_label,
                    bpc,
                    color_format: cf_str,
                    basis,
                };
            }
            DscByBandwidth::NotNeeded => {
                return DscReport {
                    state: DscState::Inactive,
                    required_gbps: required,
                    available_gbps: Some(spec.available_gbps()),
                    link_label,
                    bpc,
                    color_format: cf_str,
                    basis,
                };
            }
        }
    }

    // ③ EDID 能力补充（只说明"支持"，不能判定"启用"）
    if edid_dsc_supported == Some(true) {
        basis.push("EDID 声明支持 DSC".into());
    }

    DscReport {
        state: DscState::Unknown {
            reason: "证据不足（NVAPI 未取到链路信息）".into(),
            debug: basis.clone(),
        },
        required_gbps: required,
        available_gbps: available,
        link_label,
        bpc,
        color_format: cf_str,
        basis,
    }
}

/// 探测 NVAPI 并读取首个显示器的链路信息（feature probe，失败即 None）。
pub fn probe_nvapi_link() -> Option<(NvApi, NvLinkInfo)> {
    let api = NvApi::load().ok()?;
    let handle = api.display_handles().into_iter().next()?;
    let link = api.link_info(handle).ok()?;
    Some((api, link))
}

fn parse_cf(s: Option<&'static str>) -> ColorFormat {
    match s {
        Some("YCbCr422") => ColorFormat::YCbCr422,
        Some("YCbCr420") => ColorFormat::YCbCr420,
        Some("YCbCr444") => ColorFormat::YCbCr444,
        _ => ColorFormat::Rgb,
    }
}

fn link_spec_of(link: &NvLinkInfo) -> LinkSpec {
    let rate = link.rate_gbps;
    let lanes = link.lane_count;
    if rate >= 10.0 {
        // UHBR 档（128b/132b）
        let mut spec = if (rate - 13.5).abs() < 0.1 {
            LinkSpec::dp_uhbr13_5(lanes)
        } else if (rate - 20.0).abs() < 0.1 {
            LinkSpec::dp_uhbr20(lanes)
        } else {
            LinkSpec::dp_uhbr10(lanes)
        };
        spec.lanes = lanes;
        spec
    } else if rate >= 8.0 {
        let mut s = LinkSpec::dp_hbr3(lanes);
        s.lanes = lanes;
        s
    } else if rate >= 5.0 {
        let mut s = LinkSpec::dp_hbr2(lanes);
        s.lanes = lanes;
        s
    } else {
        let mut s = LinkSpec::dp_hbr(lanes);
        s.lanes = lanes;
        s
    }
}

fn link_available_gbps(link: &NvLinkInfo) -> f32 {
    link_spec_of(link).available_gbps()
}

fn label_of(link: &NvLinkInfo) -> String {
    let rate = link.rate_gbps;
    let prefix = if rate >= 10.0 {
        if (rate - 13.5).abs() < 0.1 { "DP2.1 UHBR13.5" }
        else if (rate - 20.0).abs() < 0.1 { "DP2.1 UHBR20" }
        else { "DP2.1 UHBR10" }
    } else if rate >= 8.0 { "DP1.4 HBR3" }
    else if rate >= 5.0 { "DP1.2 HBR2" }
    else { "DP HBR" };
    format!("{} ×{}", prefix, link.lane_count)
}

/// 解析 EDID 的 DSC 支持线索（CTA HDMI Forum VSDB / DisplayID 2.0）。
pub fn edid_dsc_support(edid: &[u8]) -> Option<bool> {
    let info = qr_edid::parse::parse(edid).ok()?;
    if info.dsc_hdmi_forum == Some(true) {
        return Some(true);
    }
    // DisplayID 2.x 存在视为支持线索（DSC 能力常在 DisplayID 数据块中）。
    if info.displayid_supported {
        return Some(true);
    }
    Some(false)
}

#[allow(dead_code)]
fn unused(_: QrError) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn uhbr10_link() -> NvLinkInfo {
        NvLinkInfo {
            lane_count: 4,
            rate_gbps: 10.0,
            bpc: Some(10),
            color_format: Some("RGB"),
            dsc_supported: None,
            dsc_enabled: None,
        }
    }

    #[test]
    fn forced_override_wins() {
        let r = detect(Some((3840, 2160, 240)), Some(&uhbr10_link()), Some(true), Some(true));
        assert!(matches!(r.state, DscState::ForcedByUser { on: true }));
    }

    #[test]
    fn nvapi_field_strong_evidence() {
        let mut link = uhbr10_link();
        link.dsc_enabled = Some(true);
        let r = detect(Some((3840, 2160, 240)), Some(&link), None, None);
        assert!(matches!(r.state, DscState::Active));
    }

    #[test]
    fn bandwidth_implies_likely_active() {
        // 3840×2160@240 10bpc RGB：~63 Gbps 未压缩，UHBR10×4 ~38 Gbps → LikelyActive。
        let r = detect(Some((3840, 2160, 240)), Some(&uhbr10_link()), None, None);
        assert!(matches!(r.state, DscState::LikelyActive { .. }));
    }

    #[test]
    fn low_bandwidth_is_inactive() {
        // 1920×1080@60：~3.2 Gbps << 38 Gbps → Inactive。
        let r = detect(Some((1920, 1080, 60)), Some(&uhbr10_link()), None, None);
        assert!(matches!(r.state, DscState::Inactive));
    }

    #[test]
    fn no_evidence_unknown() {
        let r = detect(None, None, None, None);
        assert!(matches!(r.state, DscState::Unknown { .. }));
    }

    #[test]
    fn link_label_format() {
        assert_eq!(label_of(&uhbr10_link()), "DP2.1 UHBR10 ×4");
    }
}
