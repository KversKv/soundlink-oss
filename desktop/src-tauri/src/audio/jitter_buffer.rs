//! 简单 Jitter Buffer：按 sequence 重排，吸收网络抖动；缺帧标记为 Lost（供 PLC）。
//!
//! 对齐 `docs/First/11-implementation-spec.md` §7。默认目标缓冲 80ms（8 帧）。

use crate::constants::{FRAME_DURATION_MS, FRAME_SAMPLES_TOTAL, SAMPLES_PER_FRAME_PER_CHANNEL};
use std::collections::BTreeMap;

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
    /// 目标缓冲深度（帧数）。
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
}

impl JitterBuffer {
    pub fn new(target_ms: u32) -> Self {
        let target_depth = (target_ms / FRAME_DURATION_MS as u32) as usize;
        Self {
            frames: BTreeMap::new(),
            target_depth: target_depth.max(1),
            next_play_seq: None,
            played_watermark: 0,
            packets_recv: 0,
            packets_lost: 0,
            packets_dropped: 0,
        }
    }

    /// 目标缓冲深度（帧）。
    pub fn target_depth(&self) -> usize {
        self.target_depth
    }

    /// 当前缓冲深度（帧）。
    pub fn depth(&self) -> usize {
        self.frames.len()
    }

    /// 入队一帧。
    pub fn push(&mut self, frame: JitterFrame) {
        // 过期/重复包直接丢弃。
        if self.next_play_seq.is_some() && frame.sequence < self.played_watermark {
            self.packets_dropped += 1;
            return;
        }
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
        // 取该序号：有则返回，缺口则记丢包并推进。
        if let Some(frame) = self.frames.remove(&seq) {
            self.played_watermark = seq.wrapping_add(1);
            self.next_play_seq = Some(seq.wrapping_add(1));
            PopResult::Frame(frame)
        } else {
            // 缺口：可能是还没到、也可能真丢了。
            // 判定：若缓冲里有比 seq 更大的帧，说明 seq 已丢；否则可能只是还没到（Empty）。
            if let Some(&next_have) = self.frames.keys().next() {
                if next_have > seq {
                    // seq 确实丢了。
                    self.packets_lost += 1;
                    self.played_watermark = seq.wrapping_add(1);
                    self.next_play_seq = Some(seq.wrapping_add(1));
                    PopResult::Lost
                } else {
                    // 不应出现 next_have < seq（BTreeMap 已是升序最小），保险起见 Empty。
                    PopResult::Empty
                }
            } else {
                // 缓冲空：未确定是丢包还是欠流。保守按 Empty（调用方 PLC/静音）。
                // 但若 long-running 中已播放过，则记丢包。
                if seq > 0 || self.packets_recv > 0 {
                    // 不累加 lost，避免欠流被误计为大量丢包。
                }
                PopResult::Empty
            }
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
}
