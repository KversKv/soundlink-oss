//! Jitter Buffer：按 sequence 重排，吸收网络抖动；缺帧标记为 Lost（供 PLC）。
//!
//! 阶段 4 增强（对齐 `docs/First/11-implementation-spec.md` §7、`03-audio-pipeline.md` §4）：
//! - 三档预设模式（Low / Balanced / Stable）+ 自适应（Auto）模式。
//! - 自适应：基于 inter-arrival 抖动 EWMA 动态调整目标深度。
//! - 抖动统计：jitter_ms（EWMA，RFC 3550 风格的相对时间戳抖动）。
//! - 保留过期/重复丢弃与丢包统计。

use crate::constants::{
    FRAME_DURATION_MS, FRAME_SAMPLES_TOTAL, JITTER_AUTO_BASE_FRAMES, JITTER_AUTO_K,
    JITTER_AUTO_MAX_FRAMES, JITTER_AUTO_MIN_FRAMES, JITTER_BALANCED_MS, JITTER_EWMA_ALPHA,
    JITTER_LOW_MS, JITTER_STABLE_MS, SAMPLES_PER_FRAME_PER_CHANNEL,
};
use std::collections::BTreeMap;
use std::time::Instant;

/// Jitter 模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JitterMode {
    /// 低延迟（40ms）。
    Low,
    /// 平衡（80ms，默认）。
    Balanced,
    /// 稳定（150ms，弱网）。
    Stable,
    /// 自适应：根据抖动 EWMA 动态调整。
    Auto,
}

impl JitterMode {
    pub fn from_ms(ms: u32) -> Self {
        match ms {
            x if x == JITTER_LOW_MS => JitterMode::Low,
            x if x == JITTER_STABLE_MS => JitterMode::Stable,
            _ => JitterMode::Balanced,
        }
    }

    /// 固定模式下的目标深度（帧）。Auto 模式返回初始值，后续动态调整。
    pub fn fixed_target_frames(self) -> usize {
        match self {
            JitterMode::Low => (JITTER_LOW_MS / FRAME_DURATION_MS as u32) as usize,
            JitterMode::Balanced => (JITTER_BALANCED_MS / FRAME_DURATION_MS as u32) as usize,
            JitterMode::Stable => (JITTER_STABLE_MS / FRAME_DURATION_MS as u32) as usize,
            JitterMode::Auto => (JITTER_BALANCED_MS / FRAME_DURATION_MS as u32) as usize,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            JitterMode::Low => "low",
            JitterMode::Balanced => "balanced",
            JitterMode::Stable => "stable",
            JitterMode::Auto => "auto",
        }
    }
}

/// 缓冲中的一帧（已解密的 Opus 字节）。
#[derive(Debug, Clone)]
pub struct JitterFrame {
    pub sequence: u32,
    pub timestamp: u64,
    pub data: Vec<u8>,
}

/// `pop()` 的结果。
#[derive(Debug)]
pub enum PopResult {
    /// 取到一帧。
    Frame(JitterFrame),
    /// 该序号缺失（丢包/乱序过期），调用方应做 PLC。
    Lost,
    /// 缓冲尚未预滚到位或已空且无下一帧水位。
    Empty,
}

/// Jitter Buffer。
pub struct JitterBuffer {
    /// sequence → frame。
    frames: BTreeMap<u32, JitterFrame>,
    /// 当前模式。
    mode: JitterMode,
    /// 目标缓冲深度（帧数）。Auto 模式下随抖动动态变化。
    target_depth: usize,
    /// 下一个待播放的 sequence。None = 尚未预滚。
    next_play_seq: Option<u32>,
    /// 已播放的最高 sequence（用于丢弃过期包）。
    played_watermark: u32,
    /// 统计：收到包数。
    pub packets_recv: u64,
    /// 统计：丢失包数（pop 时发现的缺口）。
    pub packets_lost: u64,
    /// 统计：过期/重复丢弃包数。
    pub packets_dropped: u64,
    /// 上一个入队包的相对到达时间戳（用于 inter-arrival 抖动计算）。
    last_arrival: Option<Instant>,
    /// 上一个入队包的 timestamp（采样计数）。
    last_rtp_timestamp: Option<u64>,
    /// inter-arrival 抖动 EWMA（帧数）。
    jitter_ewma_frames: f64,
    /// 已自适应调整次数（统计/调试用）。
    pub auto_adjustments: u64,
}

impl JitterBuffer {
    pub fn new(target_ms: u32) -> Self {
        // 直接按毫秒计算目标深度（保持向后兼容）；模式取最近预设。
        let mode = JitterMode::from_ms(target_ms);
        let target_depth = ((target_ms / FRAME_DURATION_MS as u32) as usize).max(1);
        Self::with_depth(mode, target_depth)
    }

    pub fn with_mode(mode: JitterMode) -> Self {
        let target_depth = mode.fixed_target_frames().max(1);
        Self::with_depth(mode, target_depth)
    }

    /// 用指定模式与初始目标深度构造（内部）。
    fn with_depth(mode: JitterMode, target_depth: usize) -> Self {
        Self {
            frames: BTreeMap::new(),
            mode,
            target_depth: target_depth.max(1),
            next_play_seq: None,
            played_watermark: 0,
            packets_recv: 0,
            packets_lost: 0,
            packets_dropped: 0,
            last_arrival: None,
            last_rtp_timestamp: None,
            jitter_ewma_frames: 0.0,
            auto_adjustments: 0,
        }
    }

    /// 切换模式（重置预滚与统计）。
    pub fn switch_mode(&mut self, mode: JitterMode) {
        self.mode = mode;
        self.target_depth = mode.fixed_target_frames().max(1);
        self.frames.clear();
        self.next_play_seq = None;
        self.played_watermark = 0;
        self.packets_recv = 0;
        self.packets_lost = 0;
        self.packets_dropped = 0;
        self.last_arrival = None;
        self.last_rtp_timestamp = None;
        self.jitter_ewma_frames = 0.0;
    }

    pub fn mode(&self) -> JitterMode {
        self.mode
    }

    /// 目标缓冲深度（帧）。
    pub fn target_depth(&self) -> usize {
        self.target_depth
    }

    /// 当前缓冲深度（帧）。
    pub fn depth(&self) -> usize {
        self.frames.len()
    }

    /// 抖动 EWMA（毫秒）。
    pub fn jitter_ms(&self) -> u32 {
        (self.jitter_ewma_frames * FRAME_DURATION_MS as f64) as u32
    }

    /// 入队一帧。
    pub fn push(&mut self, frame: JitterFrame) {
        // 过期/重复包直接丢弃。
        if self.next_play_seq.is_some() && frame.sequence < self.played_watermark {
            self.packets_dropped += 1;
            return;
        }
        // 抖动统计：基于 RTP timestamp 差与实际到达时间差。
        // D(i,j) = |(R_j - R_i) - (TS_j - TS_i) * frame_dur|（帧单位）
        if let (Some(prev_arr), Some(prev_ts)) = (self.last_arrival, self.last_rtp_timestamp) {
            let arr_delta = frame.timestamp.saturating_sub(prev_ts);
            // 期望到达间隔（帧）= timestamp 差 / samples_per_frame
            let expected_frames = if SAMPLES_PER_FRAME_PER_CHANNEL > 0 {
                arr_delta as f64 / SAMPLES_PER_FRAME_PER_CHANNEL as f64
            } else {
                0.0
            };
            // 实际到达间隔（帧）= wall-clock 差 / frame_duration
            let actual_frames =
                prev_arr.elapsed().as_secs_f64() * 1000.0 / FRAME_DURATION_MS as f64;
            let d = (actual_frames - expected_frames).abs();
            self.jitter_ewma_frames =
                JITTER_EWMA_ALPHA * d + (1.0 - JITTER_EWMA_ALPHA) * self.jitter_ewma_frames;
        }
        self.last_arrival = Some(Instant::now());
        self.last_rtp_timestamp = Some(frame.timestamp);

        if self.frames.insert(frame.sequence, frame).is_some() {
            self.packets_dropped += 1; // 重复序号
        } else {
            self.packets_recv += 1;
        }
        // 预滚：达到目标深度后锁定起点。
        if self.next_play_seq.is_none() && self.frames.len() >= self.target_depth {
            self.next_play_seq = self.frames.keys().next().copied();
        }
    }

    /// 拉取下一帧。调用方应按 10ms 节奏调用。
    pub fn pop(&mut self) -> PopResult {
        let Some(seq) = self.next_play_seq else {
            return PopResult::Empty;
        };
        if let Some(frame) = self.frames.remove(&seq) {
            self.played_watermark = seq.wrapping_add(1);
            self.next_play_seq = Some(seq.wrapping_add(1));
            // 自适应：每次播放后根据抖动 EWMA 调整 target_depth。
            if self.mode == JitterMode::Auto {
                self.adjust_auto_target();
            }
            PopResult::Frame(frame)
        } else {
            // 缺口判定。
            if let Some(&next_have) = self.frames.keys().next() {
                if next_have > seq {
                    self.packets_lost += 1;
                    self.played_watermark = seq.wrapping_add(1);
                    self.next_play_seq = Some(seq.wrapping_add(1));
                    if self.mode == JitterMode::Auto {
                        self.adjust_auto_target();
                    }
                    PopResult::Lost
                } else {
                    PopResult::Empty
                }
            } else {
                PopResult::Empty
            }
        }
    }

    /// 自适应目标深度：target = clamp(jitter_ewma * K + base, min, max)。
    fn adjust_auto_target(&mut self) {
        let computed = (self.jitter_ewma_frames * JITTER_AUTO_K) as usize
            + JITTER_AUTO_BASE_FRAMES;
        let new_target = computed.clamp(JITTER_AUTO_MIN_FRAMES, JITTER_AUTO_MAX_FRAMES);
        if new_target != self.target_depth {
            self.target_depth = new_target;
            self.auto_adjustments += 1;
            tracing::debug!(
                "自适应 Jitter 调整：jitter_ewma={:.1}帧 → target={}帧",
                self.jitter_ewma_frames,
                new_target
            );
        }
    }

    /// 重置（流切换/停止）。同时清零统计。
    pub fn reset(&mut self) {
        self.frames.clear();
        self.next_play_seq = None;
        self.played_watermark = 0;
        self.packets_recv = 0;
        self.packets_lost = 0;
        self.packets_dropped = 0;
        self.last_arrival = None;
        self.last_rtp_timestamp = None;
        self.jitter_ewma_frames = 0.0;
        self.auto_adjustments = 0;
        self.target_depth = self.mode.fixed_target_frames().max(1);
    }
}

/// 单帧 PCM 样本数（交错）。
pub fn frame_samples_total() -> usize {
    FRAME_SAMPLES_TOTAL
}

/// 每声道每帧样本数。
pub fn samples_per_frame_per_channel() -> usize {
    SAMPLES_PER_FRAME_PER_CHANNEL
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(seq: u32) -> JitterFrame {
        JitterFrame {
            sequence: seq,
            timestamp: seq as u64 * 480,
            data: vec![seq as u8; 16],
        }
    }

    #[test]
    fn preroll_then_in_order() {
        let mut jb = JitterBuffer::new(40); // 4 帧
                                            // 预滚期：pop 返回 Empty
        assert!(matches!(jb.pop(), PopResult::Empty));
        jb.push(frame(0));
        jb.push(frame(1));
        jb.push(frame(2));
        assert!(matches!(jb.pop(), PopResult::Empty)); // 还差一帧
        jb.push(frame(3));
        // 达到 4 帧，开始播放
        for s in 0..4u32 {
            match jb.pop() {
                PopResult::Frame(f) => assert_eq!(f.sequence, s),
                _ => panic!("期望 Frame({})", s),
            }
        }
    }

    #[test]
    fn gap_detected_as_lost() {
        let mut jb = JitterBuffer::new(20); // 2 帧
        jb.push(frame(0));
        jb.push(frame(1));
        assert!(matches!(jb.pop(), PopResult::Frame(_))); // 0
        assert!(matches!(jb.pop(), PopResult::Frame(_))); // 1
                                                          // 缺 2，入队 3 → pop 应判 2 丢失
        jb.push(frame(3));
        assert!(matches!(jb.pop(), PopResult::Lost));
        assert!(matches!(jb.pop(), PopResult::Frame(_))); // 3
        assert_eq!(jb.packets_lost, 1);
    }

    #[test]
    fn late_packet_dropped() {
        let mut jb = JitterBuffer::new(20);
        jb.push(frame(0));
        jb.push(frame(1));
        assert!(matches!(jb.pop(), PopResult::Frame(_))); // 0, watermark=1
        assert!(matches!(jb.pop(), PopResult::Frame(_))); // 1, watermark=2
                                                          // 已播放到 2，再来 0/1 → 丢弃
        jb.push(frame(0));
        assert_eq!(jb.packets_dropped, 1);
    }

    #[test]
    fn mode_switch_resets() {
        let mut jb = JitterBuffer::with_mode(JitterMode::Balanced);
        jb.push(frame(0));
        assert_eq!(jb.depth(), 1);
        jb.switch_mode(JitterMode::Stable);
        assert_eq!(jb.depth(), 0);
        assert_eq!(jb.target_depth(), 15);
    }

    #[test]
    fn auto_mode_adjusts_within_bounds() {
        let mut jb = JitterBuffer::with_mode(JitterMode::Auto);
        // 初始目标 = balanced (8 帧)。
        assert_eq!(jb.target_depth(), 8);
        // 模拟预滚后 pop，触发自适应调整（jitter=0 → target=min=4）。
        for s in 0..8 {
            jb.push(frame(s));
        }
        // 预滚后 pop 一帧。
        match jb.pop() {
            PopResult::Frame(_) => {}
            _ => panic!("期望 Frame"),
        }
        // jitter_ewma=0 → target 应降至最小值 4。
        assert_eq!(jb.target_depth(), JITTER_AUTO_MIN_FRAMES);
        assert!(jb.auto_adjustments >= 1);
    }

    #[test]
    fn jitter_ms_reports_zero_initially() {
        let jb = JitterBuffer::with_mode(JitterMode::Balanced);
        assert_eq!(jb.jitter_ms(), 0);
    }
}
