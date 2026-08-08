//! Timing 计算（display.md §7.3「Timing 生成策略」）。
//!
//! | 模式 | 用途 |
//! |---|---|
//! | `auto` | native-blanking 继承：沿用原生模式 blanking，只改 active 与 refresh（高刷兼容最好） |
//! | `cvt-rb2` | CVT-RB2：H blank 80px（8/32/40），V 3/5，min V blank 460µs |
//! | `cvt-rb3` | RB2 激进变体（V fp=1，min V blank 260µs），480Hz 级极限刷新用；**非 VESA 正式编号** |
//! | `manual` | 高级用户直填 porch/sync/polarity |

use serde::{Deserialize, Serialize};

/// 一套完整 timing 参数。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimingParams {
    pub h_active: u32,
    pub v_active: u32,
    pub h_front: u32,
    pub h_sync: u32,
    pub h_back: u32,
    pub v_front: u32,
    pub v_sync: u32,
    pub v_back: u32,
    /// true = 正极性。
    pub h_sync_pol: bool,
    pub v_sync_pol: bool,
    pub interlaced: bool,
}

impl TimingParams {
    pub fn h_total(&self) -> u32 {
        self.h_active + self.h_front + self.h_sync + self.h_back
    }
    pub fn v_total(&self) -> u32 {
        self.v_active + self.v_front + self.v_sync + self.v_back
    }
    pub fn h_blank(&self) -> u32 {
        self.h_front + self.h_sync + self.h_back
    }
    pub fn v_blank(&self) -> u32 {
        self.v_front + self.v_sync + self.v_back
    }
    /// 像素时钟（kHz）= H_total × V_total × refresh / 1000。
    pub fn pixel_clock_khz(&self, refresh_hz: u32) -> u32 {
        (self.h_total() as u64 * self.v_total() as u64 * refresh_hz as u64 / 1000) as u32
    }
    /// 行频（kHz）。
    pub fn h_freq_khz(&self, refresh_hz: u32) -> f32 {
        self.v_total() as f32 * refresh_hz as f32 / 1000.0
    }
    fn sane(&self) -> Result<(), crate::EdidErr> {
        if self.h_active == 0 || self.v_active == 0 {
            return Err(crate::EdidErr::BadTiming("active 不能为 0"));
        }
        if self.h_blank() == 0 || self.v_blank() == 0 {
            return Err(crate::EdidErr::BadTiming("blanking 不能为 0"));
        }
        Ok(())
    }
}

/// Timing 生成标准。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimingStandard {
    /// 沿用原生 blanking（推荐；无原生 timing 时回退 CVT-RB2）。
    Auto,
    CvtRb2,
    CvtRb3,
    Manual(TimingParams),
}

// CVT-RB2 常量（VESA CVT 1.2 RB2）。
const RB2_H_FP: u32 = 8;
const RB2_H_SYNC: u32 = 32;
const RB2_H_BP: u32 = 40;
const RB2_V_FP: u32 = 3;
const RB2_V_SYNC: u32 = 5;
const RB2_MIN_V_BLANK_US: f32 = 460.0;
// RB3（RB2 激进变体，见模块文档注释）。
const RB3_V_FP: u32 = 1;
const RB3_MIN_V_BLANK_US: f32 = 260.0;

/// CVT-RB 系生成（v2/v3 仅 V 参数与最小 blank 不同）。
fn cvt_rb(h: u32, v: u32, refresh_hz: u32, v3: bool) -> Result<TimingParams, crate::EdidErr> {
    if h == 0 || v == 0 || refresh_hz == 0 {
        return Err(crate::EdidErr::BadTiming("分辨率/刷新率不能为 0"));
    }
    let h_total = h + RB2_H_FP + RB2_H_SYNC + RB2_H_BP;
    let (v_fp, v_sync, min_blank) = if v3 {
        (RB3_V_FP, RB2_V_SYNC, RB3_MIN_V_BLANK_US)
    } else {
        (RB2_V_FP, RB2_V_SYNC, RB2_MIN_V_BLANK_US)
    };
    // 行时间（µs）= H_total / pixel_clock_MHz；pixel_clock = H_total × V_total × Hz。
    // 迭代：先估 v_total，再精算 back porch。
    let mut v_back = 6u32;
    for _ in 0..16 {
        let v_total = v + v_fp + v_sync + v_back;
        let pix_hz = h_total as f64 * v_total as f64 * refresh_hz as f64;
        if pix_hz <= 0.0 {
            return Err(crate::EdidErr::BadTiming("像素时钟为 0"));
        }
        let line_us = (h_total as f64) / (pix_hz / 1e6);
        let blank_us = line_us * (v_fp + v_sync + v_back) as f64;
        if blank_us >= min_blank as f64 {
            break;
        }
        // 还差多少行
        let missing = ((min_blank as f64 - blank_us) / line_us).ceil() as u32;
        v_back = v_back.saturating_add(missing.max(1));
    }
    let t = TimingParams {
        h_active: h,
        v_active: v,
        h_front: RB2_H_FP,
        h_sync: RB2_H_SYNC,
        h_back: RB2_H_BP,
        v_front: v_fp,
        v_sync,
        v_back,
        // RB 系惯例：HSYNC- / VSYNC+。
        h_sync_pol: false,
        v_sync_pol: true,
        interlaced: false,
    };
    t.sane()?;
    Ok(t)
}

/// native-blanking 继承：沿用原生 timing 的 H/V total，只改 active 与 refresh。
///
/// 原生 blanking 不足以容纳新 active（total - active < 最小 blank）时回退 CVT-RB2。
fn inherit_native(
    h: u32,
    v: u32,
    refresh_hz: u32,
    native: &TimingParams,
) -> Result<TimingParams, crate::EdidErr> {
    // 新 active 不能超过原生 total 减去最小 blank（H≥80, V≥6）。
    if h + 80 > native.h_total() || v + 6 > native.v_total() || native.h_blank() < 80 {
        return cvt_rb(h, v, refresh_hz, false);
    }
    let t = TimingParams {
        h_active: h,
        v_active: v,
        // 保持原生比例结构：fp/sync 不变，back porch 吸收差值。
        h_front: native.h_front,
        h_sync: native.h_sync,
        h_back: native.h_total() - h - native.h_front - native.h_sync,
        v_front: native.v_front,
        v_sync: native.v_sync,
        v_back: native.v_total() - v - native.v_front - native.v_sync,
        h_sync_pol: native.h_sync_pol,
        v_sync_pol: native.v_sync_pol,
        interlaced: false,
    };
    t.sane()?;
    Ok(t)
}

/// 生成 timing。`native` 为显示器原生（最高像素时钟）timing，`Auto` 时使用。
pub fn generate(
    standard: TimingStandard,
    h: u32,
    v: u32,
    refresh_hz: u32,
    native: Option<&TimingParams>,
) -> Result<TimingParams, crate::EdidErr> {
    match standard {
        TimingStandard::Auto => match native {
            Some(n) => inherit_native(h, v, refresh_hz, n),
            None => cvt_rb(h, v, refresh_hz, false),
        },
        TimingStandard::CvtRb2 => cvt_rb(h, v, refresh_hz, false),
        TimingStandard::CvtRb3 => cvt_rb(h, v, refresh_hz, true),
        TimingStandard::Manual(t) => {
            if t.h_active != h || t.v_active != v {
                return Err(crate::EdidErr::BadTiming("manual timing 的 active 与目标分辨率不一致"));
            }
            t.sane()?;
            Ok(t)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cvt_rb2_1920x1440_480_totals() {
        // 文档 §6.1 估算 H_total≈2000 ✓；V_total≈1471 是文档粗估——RB2 的 460µs
        // 最小 V blank 在 480Hz 下实际需要 ≈1849 行（blanking 约占帧时间 22%）。
        let t = generate(TimingStandard::CvtRb2, 1920, 1440, 480, None).unwrap();
        assert_eq!(t.h_total(), 2000);
        let line_us = t.h_total() as f64 / (t.pixel_clock_khz(480) as f64 / 1000.0);
        let blank_us = line_us * t.v_blank() as f64;
        assert!(blank_us >= 460.0, "blank_us={blank_us}");
        // 收敛值应在 1849 附近（±30 行容差防迭代步进抖动）。
        assert!((1819..=1879).contains(&t.v_total()), "v_total={}", t.v_total());
    }

    #[test]
    fn cvt_rb3_more_aggressive_than_rb2() {
        let rb2 = generate(TimingStandard::CvtRb2, 1920, 1440, 480, None).unwrap();
        let rb3 = generate(TimingStandard::CvtRb3, 1920, 1440, 480, None).unwrap();
        assert!(rb3.v_total() < rb2.v_total());
        assert!(rb3.pixel_clock_khz(480) < rb2.pixel_clock_khz(480));
    }

    #[test]
    fn inherit_native_keeps_totals() {
        // 原生 3840×2160@240：假设 H_total=4000, V_total=2222（blanking 160/62）。
        let native = TimingParams {
            h_active: 3840,
            v_active: 2160,
            h_front: 48,
            h_sync: 32,
            h_back: 80,
            v_front: 3,
            v_sync: 5,
            v_back: 54,
            h_sync_pol: false,
            v_sync_pol: true,
            interlaced: false,
        };
        let t = generate(TimingStandard::Auto, 1920, 1440, 480, Some(&native)).unwrap();
        // total 保持原生，active 变更，back porch 吸收差值。
        assert_eq!(t.h_total(), native.h_total());
        assert_eq!(t.v_total(), native.v_total());
        assert_eq!(t.h_active, 1920);
        assert_eq!(t.v_active, 1440);
    }

    #[test]
    fn inherit_native_falls_back_when_too_big() {
        // 原生 total 太小放不下 1920+80 → 回退 RB2。
        let native = TimingParams {
            h_active: 1024,
            v_active: 768,
            h_front: 24,
            h_sync: 136,
            h_back: 160,
            v_front: 3,
            v_sync: 6,
            v_back: 29,
            h_sync_pol: false,
            v_sync_pol: false,
            interlaced: false,
        };
        let t = generate(TimingStandard::Auto, 1920, 1440, 480, Some(&native)).unwrap();
        assert_eq!(t.h_total(), 2000); // RB2 结构
    }

    #[test]
    fn manual_requires_matching_active() {
        let mut t = generate(TimingStandard::CvtRb2, 1920, 1080, 144, None).unwrap();
        let ok = generate(TimingStandard::Manual(t), 1920, 1080, 144, None);
        assert!(ok.is_ok());
        t.h_active = 1280;
        let bad = generate(TimingStandard::Manual(t), 1920, 1080, 144, None);
        assert!(bad.is_err());
    }

    #[test]
    fn pixel_clock_calc() {
        let t = TimingParams {
            h_active: 1920,
            v_active: 1080,
            h_front: 8,
            h_sync: 32,
            h_back: 40,
            v_front: 3,
            v_sync: 5,
            v_back: 20,
            h_sync_pol: false,
            v_sync_pol: true,
            interlaced: false,
        };
        // 2000 × 1108 × 144 / 1000 = 319,104 kHz。
        assert_eq!(t.pixel_clock_khz(144), 319_104);
    }
}
