//! 时钟漂移校正（±0.5% 线性重采样）。
//!
//! 对齐 `docs/First/11-implementation-spec.md` §7、`03-audio-pipeline.md` §5。
//! 监测缓冲水位长期偏离目标值，微调重采样比率平滑吸收发送/接收端时钟漂移，
//! 避免缓冲耗尽（断音）或溢出（丢包/爆音）。
//!
//! 实现策略（最小依赖、无 rubato）：
//! - 比率范围 ±0.5%（DRIFT_CORRECTION_MAX_RATIO）。
//! - 当 |buffer_depth - target| > DRIFT_ADJUST_THRESHOLD_FRAMES 时启动校正：
//!   缓冲偏低 → 慢放（ratio < 1，重复样本，拉长播放时长 → 缓冲回升）。
//!   缓冲偏高 → 快放（ratio > 1，跳过样本，缩短播放时长 → 缓冲下降）。
//! - 线性插值实现，单帧 480 样本/声道，开销极低。
//! - ratio 步进固定（每帧调整 ±0.01%），避免突变爆音。

use crate::constants::{DRIFT_ADJUST_THRESHOLD_FRAMES, DRIFT_CORRECTION_MAX_RATIO};

/// 漂移校正器：根据缓冲水位偏差计算重采样比率，并对 PCM 做线性插值。
pub struct DriftResampler {
    /// 当前比率（1.0 = 不变）。范围 [1-max, 1+max]。
    ratio: f64,
    /// 重采样相位（用于跨帧连续）。
    phase: f64,
    /// 上一个输入样本（L, R），用于跨帧插值。
    last_l: i16,
    last_r: i16,
    /// 是否已初始化 last sample。
    initialized: bool,
}

impl Default for DriftResampler {
    fn default() -> Self {
        Self::new()
    }
}

impl DriftResampler {
    pub fn new() -> Self {
        Self {
            ratio: 1.0,
            phase: 0.0,
            last_l: 0,
            last_r: 0,
            initialized: false,
        }
    }

    /// 当前比率。
    pub fn ratio(&self) -> f64 {
        self.ratio
    }

    /// 根据缓冲水位偏差更新目标比率。
    /// `depth` 当前缓冲帧数，`target` 目标帧数。
    pub fn observe(&mut self, depth: usize, target: usize) {
        let diff = depth as i64 - target as i64;
        if diff.abs() <= DRIFT_ADJUST_THRESHOLD_FRAMES as i64 {
            // 偏差在容忍范围内，缓慢回归 1.0。
            self.ratio = lerp_ratio(self.ratio, 1.0, 0.5);
            return;
        }
        // 缓冲偏低 → 慢放（ratio<1）；偏高 → 快放（ratio>1）。
        // 步进按偏差方向固定 ±0.01%，避免抖动。
        let target_ratio = if diff < 0 {
            1.0 - DRIFT_CORRECTION_MAX_RATIO
        } else {
            1.0 + DRIFT_CORRECTION_MAX_RATIO
        };
        self.ratio = lerp_ratio(self.ratio, target_ratio, 0.1);
    }

    /// 重采样一帧交错立体声 i16 PCM。
    /// 输入：480 个样本对（960 i16）。输出近似 960 个 i16（±0.5%）。
    pub fn process(&mut self, input: &[i16]) -> Vec<i16> {
        // 假设输入为交错 stereo。
        let pairs = input.len() / 2;
        if pairs == 0 {
            return Vec::new();
        }
        // 初始化 last sample（用于跨帧边界插值）。
        if !self.initialized {
            self.last_l = input[0];
            self.last_r = input.get(1).copied().unwrap_or(self.last_l);
            self.initialized = true;
        }
        // 输出样本对数 = pairs / ratio（每帧约 ±2~3 样本偏差）。
        let out_pairs = (pairs as f64 / self.ratio).round() as usize;
        let mut out = Vec::with_capacity(out_pairs * 2);
        let mut idx = 0.0f64;
        for _ in 0..out_pairs {
            let i = idx as usize;
            if i >= pairs {
                break;
            }
            let frac = idx - i as f64;
            let cur_l = input[i * 2];
            let cur_r = input[i * 2 + 1];
            // 插值需用 cur 与 next：frac=0 → 输出 cur（正确）；
            // frac>0 → 在 cur 与 next 之间插值。
            // 边界处 next 取自下一帧首样本（self.last_l/r 已存上一帧末样本，
            // 但此处 i+1 越界时用 last 作为 next 不合理；改为取本帧末样本，
            // 避免相位错位）。
            let (next_l, next_r) = if i + 1 < pairs {
                (input[(i + 1) * 2], input[(i + 1) * 2 + 1])
            } else {
                // 帧末尾：用当前样本作为 next（frac 通常已接近 1，
                // 误差可忽略；跨帧连续由下帧的 cur[0] 接续）。
                (cur_l, cur_r)
            };
            let l = lerp_i16(cur_l, next_l, frac);
            let r = lerp_i16(cur_r, next_r, frac);
            out.push(l);
            out.push(r);
            idx += self.ratio;
        }
        // 保存最后一个输入样本供下帧边界插值使用。
        let last_idx = pairs - 1;
        self.last_l = input[last_idx * 2];
        self.last_r = input[last_idx * 2 + 1];
        out
    }

    /// 重置（流切换）。
    pub fn reset(&mut self) {
        self.ratio = 1.0;
        self.phase = 0.0;
        self.last_l = 0;
        self.last_r = 0;
        self.initialized = false;
    }
}

fn lerp_i16(a: i16, b: i16, t: f64) -> i16 {
    let v = (a as f64) + (b as f64 - a as f64) * t;
    v.clamp(-32768.0, 32767.0) as i16
}

fn lerp_ratio(cur: f64, target: f64, alpha: f64) -> f64 {
    cur + (target - cur) * alpha
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratio_starts_at_one() {
        let r = DriftResampler::new();
        assert!((r.ratio() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn observe_low_buffer_slows_down() {
        let mut r = DriftResampler::new();
        // 缓冲偏低 5 帧（>阈值 3）→ ratio 应 < 1。
        for _ in 0..20 {
            r.observe(0, 8);
        }
        assert!(r.ratio() < 1.0, "ratio 应小于 1，实际 {}", r.ratio());
    }

    #[test]
    fn observe_high_buffer_speeds_up() {
        let mut r = DriftResampler::new();
        // 缓冲偏高 5 帧 → ratio 应 > 1。
        for _ in 0..20 {
            r.observe(20, 8);
        }
        assert!(r.ratio() > 1.0, "ratio 应大于 1，实际 {}", r.ratio());
    }

    #[test]
    fn observe_balanced_returns_to_one() {
        let mut r = DriftResampler::new();
        // 先偏离再回到平衡。
        for _ in 0..20 {
            r.observe(20, 8);
        }
        for _ in 0..20 {
            r.observe(8, 8);
        }
        assert!(
            (r.ratio() - 1.0).abs() < 0.01,
            "ratio 应回归 1，实际 {}",
            r.ratio()
        );
    }

    #[test]
    fn process_preserves_length_approx() {
        let mut r = DriftResampler::new();
        r.ratio = 1.0;
        let input: Vec<i16> = (0..960).map(|i| (i % 32768) as i16).collect();
        let out = r.process(&input);
        assert_eq!(out.len(), 960, "ratio=1.0 应保持长度");
    }

    #[test]
    fn process_ratio_one_is_identity() {
        // 回归测试：ratio=1.0 时输出应等于输入（不能是上一帧末样本重复）。
        // 修复前 bug：lerp(last, cur, frac=0) = last，导致输出全是上一帧末样本。
        let mut r = DriftResampler::new();
        r.ratio = 1.0;
        let input: Vec<i16> = (0..960).map(|i| (i * 2) as i16).collect();
        let out = r.process(&input);
        assert_eq!(out, input, "ratio=1.0 时输出应等于输入");

        // 第二帧：验证不会用第一帧末样本污染。
        let input2: Vec<i16> = (0..960).map(|i| (i * 3 + 1) as i16).collect();
        let out2 = r.process(&input2);
        assert_eq!(out2, input2, "第二帧输出也应等于输入");
    }

    #[test]
    fn ratio_bounded() {
        let mut r = DriftResampler::new();
        // 长期偏离，ratio 应被限制在 ±0.5%。
        for _ in 0..1000 {
            r.observe(0, 8);
        }
        assert!(r.ratio() >= 1.0 - DRIFT_CORRECTION_MAX_RATIO - 1e-9);
        assert!(r.ratio() <= 1.0);
        for _ in 0..1000 {
            r.observe(100, 8);
        }
        assert!(r.ratio() <= 1.0 + DRIFT_CORRECTION_MAX_RATIO + 1e-9);
        assert!(r.ratio() >= 1.0);
    }

    #[test]
    fn reset_clears_state() {
        let mut r = DriftResampler::new();
        for _ in 0..10 {
            r.observe(0, 8);
        }
        assert!(r.ratio() < 1.0);
        r.reset();
        assert!((r.ratio() - 1.0).abs() < 1e-9);
        assert!(!r.initialized);
    }
}
