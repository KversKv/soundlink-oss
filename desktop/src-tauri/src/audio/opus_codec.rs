//! Opus 编解码抽象 + 实现。
//!
//! - `AudioCodec` trait：统一 encode / decode / PLC 接口。
//! - 默认 `opus` feature 启用真实 libopus（48kHz/Stereo/10ms/128kbps）。
//! - 关闭 `opus` feature 时回退 `PassthroughCodec`：把 PCM(i16) 原样当作"帧"，
//!   仅用于无法编译 libopus 时验证 UDP/加密/Jitter/输出链路（非 spec 合规，
//!   仅供开发自测）。生产须启用 `opus`。

use crate::constants::{CHANNELS, SAMPLES_PER_FRAME_PER_CHANNEL};

/// 音频编解码接口（单帧 10ms）。
pub trait AudioCodec: Send {
    /// 编码一帧 PCM（交错 i16，长度 = 480*channels）→ Opus 字节。
    fn encode(&mut self, pcm: &[i16]) -> Vec<u8>;
    /// 解码一帧 Opus 字节 → PCM（交错 i16）。
    fn decode(&mut self, frame: &[u8]) -> Vec<i16>;
    /// 丢包补偿（PLC）：生成一帧舒适帧。
    fn decode_plc(&mut self) -> Vec<i16>;
}

/// 单帧 PCM 样本数（交错）。
pub fn frame_pcm_len() -> usize {
    SAMPLES_PER_FRAME_PER_CHANNEL * CHANNELS as usize
}

// ───────────────────────── passthrough（开发回退） ─────────────────────────

/// 透传"编解码"：i16 PCM ↔ 小端字节。不压缩，仅用于无 libopus 时的链路自测。
pub struct PassthroughCodec;

impl Default for PassthroughCodec {
    fn default() -> Self {
        Self
    }
}

impl PassthroughCodec {
    pub fn new() -> Self {
        Self
    }
}

impl AudioCodec for PassthroughCodec {
    fn encode(&mut self, pcm: &[i16]) -> Vec<u8> {
        let mut out = Vec::with_capacity(pcm.len() * 2);
        for &s in pcm {
            out.extend_from_slice(&s.to_le_bytes());
        }
        out
    }
    fn decode(&mut self, frame: &[u8]) -> Vec<i16> {
        let mut out = Vec::with_capacity(frame.len() / 2);
        for chunk in frame.chunks_exact(2) {
            out.push(i16::from_le_bytes([chunk[0], chunk[1]]));
        }
        out
    }
    fn decode_plc(&mut self) -> Vec<i16> {
        vec![0i16; frame_pcm_len()]
    }
}

// ───────────────────────── libopus 真实实现（FFI via libopus_sys） ─────────────────────────

#[cfg(feature = "opus")]
pub mod libopus {
    use super::{frame_pcm_len, AudioCodec};
    use crate::constants::{OPUS_BITRATE, SAMPLES_PER_FRAME_PER_CHANNEL, SAMPLE_RATE};
    use libopus_sys as opusffi;
    use std::os::raw::{c_int, c_uchar};
    use std::ptr;

    const OPUS_SET_BITRATE_REQUEST: c_int = 4002;
    const OPUS_SIGNAL_REQUEST: c_int = 4024;
    const OPUS_SIGNAL_MUSIC: c_int = 3002;

    /// libopus 编解码器封装。
    pub struct LibopusCodec {
        encoder: *mut opusffi::OpusEncoder,
        decoder: *mut opusffi::OpusDecoder,
        enc_buf: Vec<u8>,
    }

    unsafe impl Send for LibopusCodec {}

    #[derive(Debug)]
    pub enum OpusError {
        Create(i32),
    }

    impl LibopusCodec {
        pub fn new() -> Result<Self, OpusError> {
            unsafe {
                let mut err: c_int = 0;
                let enc = opusffi::opus_encoder_create(
                    SAMPLE_RATE as c_int,
                    2,
                    opusffi::OPUS_APPLICATION_AUDIO as c_int,
                    &mut err,
                );
                if err != 0 || enc.is_null() {
                    return Err(OpusError::Create(err));
                }
                opusffi::opus_encoder_ctl(enc, OPUS_SET_BITRATE_REQUEST, OPUS_BITRATE as c_int);
                opusffi::opus_encoder_ctl(enc, OPUS_SIGNAL_REQUEST, OPUS_SIGNAL_MUSIC);

                let dec = opusffi::opus_decoder_create(SAMPLE_RATE as c_int, 2, &mut err);
                if err != 0 || dec.is_null() {
                    opusffi::opus_encoder_destroy(enc);
                    return Err(OpusError::Create(err));
                }
                Ok(Self {
                    encoder: enc,
                    decoder: dec,
                    enc_buf: vec![0u8; 4000],
                })
            }
        }
    }

    impl Drop for LibopusCodec {
        fn drop(&mut self) {
            unsafe {
                if !self.encoder.is_null() {
                    opusffi::opus_encoder_destroy(self.encoder);
                }
                if !self.decoder.is_null() {
                    opusffi::opus_decoder_destroy(self.decoder);
                }
            }
        }
    }

    impl AudioCodec for LibopusCodec {
        fn encode(&mut self, pcm: &[i16]) -> Vec<u8> {
            unsafe {
                let n = opusffi::opus_encode(
                    self.encoder,
                    pcm.as_ptr(),
                    SAMPLES_PER_FRAME_PER_CHANNEL as c_int,
                    self.enc_buf.as_mut_ptr() as *mut c_uchar,
                    self.enc_buf.len() as c_int,
                );
                if n > 0 {
                    self.enc_buf[..n as usize].to_vec()
                } else {
                    Vec::new()
                }
            }
        }
        fn decode(&mut self, frame: &[u8]) -> Vec<i16> {
            let mut out = vec![0i16; frame_pcm_len()];
            unsafe {
                let n = opusffi::opus_decode(
                    self.decoder,
                    if frame.is_empty() {
                        ptr::null()
                    } else {
                        frame.as_ptr() as *const c_uchar
                    },
                    frame.len() as c_int,
                    out.as_mut_ptr(),
                    SAMPLES_PER_FRAME_PER_CHANNEL as c_int,
                    0,
                );
                if n < 0 {
                    return vec![0i16; frame_pcm_len()];
                }
            }
            out
        }
        fn decode_plc(&mut self) -> Vec<i16> {
            let mut out = vec![0i16; frame_pcm_len()];
            unsafe {
                let _ = opusffi::opus_decode(
                    self.decoder,
                    ptr::null(),
                    0,
                    out.as_mut_ptr(),
                    SAMPLES_PER_FRAME_PER_CHANNEL as c_int,
                    0,
                );
            }
            out
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn roundtrip() {
            let mut c = LibopusCodec::new().unwrap();
            let pcm: Vec<i16> = (0..frame_pcm_len())
                .map(|i| ((i as f32 * 0.1).sin() * 8000.0) as i16)
                .collect();
            let frame = c.encode(&pcm);
            assert!(!frame.is_empty());
            let pcm2 = c.decode(&frame);
            assert_eq!(pcm2.len(), pcm.len());
        }
    }
}

/// 构造默认编解码器：优先 libopus，回退 passthrough。
pub fn default_codec() -> Box<dyn AudioCodec> {
    #[cfg(feature = "opus")]
    {
        match libopus::LibopusCodec::new() {
            Ok(c) => return Box::new(c),
            Err(e) => {
                tracing::warn!("libopus 初始化失败，回退 passthrough：{:?}", e);
            }
        }
    }
    Box::new(PassthroughCodec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passthrough_roundtrip() {
        let mut c = PassthroughCodec::new();
        let pcm: Vec<i16> = (0..frame_pcm_len() as i16)
            .map(|i| i.wrapping_mul(100))
            .collect();
        let f = c.encode(&pcm);
        let pcm2 = c.decode(&f);
        assert_eq!(pcm, pcm2);
        assert_eq!(c.decode_plc().len(), frame_pcm_len());
    }
}
