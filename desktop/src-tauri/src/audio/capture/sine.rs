//! 正弦波测试采集源（跨平台）。
//!
//! 生成 440Hz（可配置）立体声 PCM i16，供 Sender 自测使用。
//! 与 `examples/loopback_sender.rs` 的 `gen_440hz_frame` 同源逻辑。

use super::CaptureSource;
use crate::audio::opus_codec::frame_pcm_len;
use crate::constants::SAMPLES_PER_FRAME_PER_CHANNEL;
use std::f32::consts::TAU;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// 正弦波采集源。
pub struct SineWaveCapture {
    freq: f32,
    amp: f32,
    sample_offset: Mutex<u64>,
    running: AtomicBool,
}

impl SineWaveCapture {
    /// 创建指定频率/振幅（0..1）的正弦源。
    pub fn new(freq: f32, amp: f32) -> Self {
        Self {
            freq,
            amp: amp.clamp(0.0, 1.0),
            sample_offset: Mutex::new(0),
            running: AtomicBool::new(false),
        }
    }
}

impl CaptureSource for SineWaveCapture {
    fn name(&self) -> &str {
        "Sine 440Hz"
    }

    fn start(&mut self) -> Result<(), String> {
        self.running.store(true, Ordering::SeqCst);
        *self.sample_offset.lock().unwrap() = 0;
        Ok(())
    }

    fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
    }

    fn poll_frame(&mut self) -> Option<Vec<i16>> {
        if !self.running.load(Ordering::SeqCst) {
            return None;
        }
        let sr = crate::constants::SAMPLE_RATE as f32;
        let mut offset = self.sample_offset.lock().unwrap();
        let base = *offset;
        *offset += SAMPLES_PER_FRAME_PER_CHANNEL as u64;

        let mut pcm = Vec::with_capacity(frame_pcm_len());
        for i in 0..SAMPLES_PER_FRAME_PER_CHANNEL {
            let t = (base + i as u64) as f32 / sr;
            let v = (t * self.freq * TAU).sin() * self.amp;
            let s = (v * 32767.0).clamp(-32768.0, 32767.0) as i16;
            pcm.push(s); // L
            pcm.push(s); // R
        }
        Some(pcm)
    }

    fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produces_frames_when_running() {
        let mut s = SineWaveCapture::new(440.0, 0.25);
        s.start().unwrap();
        let f = s.poll_frame().unwrap();
        assert_eq!(f.len(), frame_pcm_len());
        // 同一 offset 下相同输入应产生相同帧。
        let mut s2 = SineWaveCapture::new(440.0, 0.25);
        s2.start().unwrap();
        let f2 = s2.poll_frame().unwrap();
        assert_eq!(f, f2);
    }

    #[test]
    fn no_frame_when_stopped() {
        let mut s = SineWaveCapture::new(440.0, 0.25);
        assert!(s.poll_frame().is_none());
        s.start().unwrap();
        assert!(s.poll_frame().is_some());
        s.stop();
        assert!(s.poll_frame().is_none());
    }
}
