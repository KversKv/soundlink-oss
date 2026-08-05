//! 会话格式转换（阶段 P · 参数动态化）。
//!
//! 采集/播放始终工作于设备基线（48kHz/Stereo），会话格式（44.1k/Mono/20ms 等）
//! 仅在编码前与解码后做轻量转换：
//! - `SessionConverter::to_session`：基线 PCM → 会话格式（发送端编码前）。
//! - `SessionConverter::to_baseline`：会话格式 PCM → 基线（接收端解码后）。
//!
//! 设计取舍（最小依赖、无 rubato）：
//! - 重采样用线性插值（质量对语音/音乐足够，开销极低，延迟可控）。
//! - 声道：Stereo→Mono 取平均；Mono→Stereo 复制。
//! - 基线↔基线（无格式差异）时零拷贝直通，不引入任何额外延迟。

use crate::constants::AudioFormat;

/// 基线格式（设备侧固定）。
const BASELINE: AudioFormat = AudioFormat {
    sample_rate: crate::constants::SAMPLE_RATE,
    channels: crate::constants::CHANNELS,
    frame_duration_ms: crate::constants::FRAME_DURATION_MS,
};

/// 会话格式转换器（持有会话格式与跨帧重采样相位）。
pub struct SessionConverter {
    session: AudioFormat,
}

impl SessionConverter {
    /// 构造。`session` 会先过白名单归一化。
    pub fn new(session: AudioFormat) -> Self {
        Self {
            session: session.normalized(),
        }
    }

    /// 会话格式。
    pub fn session_format(&self) -> AudioFormat {
        self.session
    }

    /// 是否需要转换（会话格式 == 基线时直通）。
    pub fn is_passthrough(&self) -> bool {
        self.session == BASELINE
    }

    /// 发送端：基线交错 PCM → 会话格式交错 PCM。
    /// 输入长度应为基线一帧（frame_samples_total）。
    pub fn to_session(&self, baseline_pcm: &[i16]) -> Vec<i16> {
        convert(baseline_pcm, &BASELINE, &self.session)
    }

    /// 接收端：会话格式交错 PCM → 基线交错 PCM。
    /// 输入长度应为会话一帧（frame_samples_total）。
    pub fn to_baseline(&self, session_pcm: &[i16]) -> Vec<i16> {
        convert(session_pcm, &self.session, &BASELINE)
    }
}

/// 通用转换：声道映射 → 线性重采样。输入/输出均为交错 i16。
fn convert(input: &[i16], from: &AudioFormat, to: &AudioFormat) -> Vec<i16> {
    if from == to {
        return input.to_vec();
    }
    // 1) 声道映射到目标声道数。
    let channel_mapped = map_channels(input, from.channels, to.channels);
    // 2) 重采样到目标采样率。
    resample_linear(&channel_mapped, from.sample_rate, to.sample_rate, to.channels)
}

/// 声道映射。interleave 语义：每帧 = channels 个样本。
fn map_channels(input: &[i16], from_ch: u8, to_ch: u8) -> Vec<i16> {
    if from_ch == to_ch {
        return input.to_vec();
    }
    let frames = input.len() / from_ch as usize;
    let mut out = Vec::with_capacity(frames * to_ch as usize);
    match (from_ch, to_ch) {
        (2, 1) => {
            // Stereo → Mono：平均。
            for f in 0..frames {
                let l = input[f * 2] as i32;
                let r = input[f * 2 + 1] as i32;
                out.push(((l + r) / 2) as i16);
            }
        }
        (1, 2) => {
            // Mono → Stereo：复制。
            for &m in input.iter().take(frames) {
                out.push(m);
                out.push(m);
            }
        }
        _ => return input.to_vec(), // 未支持的组合直通
    }
    out
}

/// 线性插值重采样（按目标采样率重算样本数）。
/// ratio = to_rate / from_rate；输出帧数 = round(in_frames * ratio)。
fn resample_linear(input: &[i16], from_rate: u32, to_rate: u32, channels: u8) -> Vec<i16> {
    if from_rate == to_rate {
        return input.to_vec();
    }
    let ch = channels as usize;
    let in_frames = input.len() / ch;
    if in_frames == 0 {
        return Vec::new();
    }
    let ratio = to_rate as f64 / from_rate as f64;
    let out_frames = ((in_frames as f64) * ratio).round() as usize;
    let mut out = Vec::with_capacity(out_frames * ch);
    for of in 0..out_frames {
        // 源浮点位置。
        let src_pos = of as f64 / ratio;
        let i0 = src_pos.floor() as usize;
        let i1 = (i0 + 1).min(in_frames - 1);
        let frac = (src_pos - i0 as f64) as f32;
        for c in 0..ch {
            let a = input[i0 * ch + c] as f32;
            let b = input[i1 * ch + c] as f32;
            out.push((a + (b - a) * frac) as i16);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passthrough_when_baseline() {
        let conv = SessionConverter::new(AudioFormat::default());
        assert!(conv.is_passthrough());
        let pcm = vec![1i16, 2, 3, 4];
        assert_eq!(conv.to_session(&pcm), pcm);
        assert_eq!(conv.to_baseline(&pcm), pcm);
    }

    #[test]
    fn stereo_to_mono_halves_and_averages() {
        // 基线 960 样本（480 帧立体声）→ mono 480 样本。
        let mut pcm = Vec::new();
        for i in 0..480 {
            pcm.push(1000i16);
            pcm.push(3000i16);
            let _ = i;
        }
        let conv = SessionConverter::new(AudioFormat {
            sample_rate: 48_000,
            channels: 1,
            frame_duration_ms: 10,
        });
        let out = conv.to_session(&pcm);
        assert_eq!(out.len(), 480);
        assert!(out.iter().all(|&s| s == 2000));
    }

    #[test]
    fn mono_to_stereo_duplicates() {
        let pcm = vec![100i16; 480];
        let out = map_channels(&pcm, 1, 2);
        assert_eq!(out.len(), 960);
        assert!(out.iter().all(|&s| s == 100));
    }

    #[test]
    fn resample_48k_to_441k_changes_length() {
        // 480 帧 @48k → 441 帧 @44.1k（单声道）。
        let pcm = vec![0i16; 480];
        let out = resample_linear(&pcm, 48_000, 44_100, 1);
        assert_eq!(out.len(), 441);
    }

    #[test]
    fn resample_roundtrip_preserves_dc() {
        // 直流信号重采样后幅度不变。
        let pcm = vec![5000i16; 960];
        let out = resample_linear(&pcm, 48_000, 44_100, 2);
        assert!(out.iter().all(|&s| (s - 5000).abs() <= 1));
    }

    #[test]
    fn full_mono_roundtrip() {
        // 基线 → mono（48k）→ 基线，长度还原、直流幅度保持。
        // 注：会话采样率固定 48kHz（Opus 限制），动态化维度为声道/帧长。
        let conv = SessionConverter::new(AudioFormat {
            sample_rate: 48_000,
            channels: 1,
            frame_duration_ms: 10,
        });
        let baseline = vec![2000i16; 960];
        let session = conv.to_session(&baseline);
        // 48k mono 10ms = 480 样本。
        assert_eq!(session.len(), 480);
        let back = conv.to_baseline(&session);
        assert!(back.iter().all(|&s| (s - 2000).abs() <= 2));
    }

    #[test]
    fn non_whitelist_rate_falls_back_to_baseline() {
        // 44.1k 不在白名单（Opus 不支持），normalized 回退 48kHz。
        let f = AudioFormat {
            sample_rate: 44_100,
            channels: 2,
            frame_duration_ms: 10,
        }
        .normalized();
        assert_eq!(f.sample_rate, 48_000);
    }
}
