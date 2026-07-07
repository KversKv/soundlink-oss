//! Windows WASAPI Loopback 采集（阶段 5）。
//!
//! 通过 `IAudioClient` + `AUDCLNT_STREAMFLAGS_LOOPBACK` 采集系统播放音频，
//! 归一化到 48kHz/Stereo/Int16 交错，供 Sender 编码发送。
//!
//! 对齐 `docs/First/08-platform-notes.md` §4、`03-audio-pipeline.md` §1。
//!
//! 采集在独立线程（COM MTA）运行，通过环形缓冲向 `poll_frame` 供数。

use super::CaptureSource;
use crate::constants::{CAPTURE_RING_FRAMES, FRAME_SAMPLES_TOTAL, SAMPLE_RATE};
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

/// WASAPI loopback 采集源。
pub struct WasapiLoopbackCapture {
    running: Arc<AtomicBool>,
    ring: Arc<Mutex<VecDeque<i16>>>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl WasapiLoopbackCapture {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            ring: Arc::new(Mutex::new(VecDeque::with_capacity(
                CAPTURE_RING_FRAMES * FRAME_SAMPLES_TOTAL,
            ))),
            thread: Mutex::new(None),
        }
    }
}

impl Default for WasapiLoopbackCapture {
    fn default() -> Self {
        Self::new()
    }
}

impl CaptureSource for WasapiLoopbackCapture {
    fn name(&self) -> &str {
        "WASAPI Loopback"
    }

    fn start(&mut self) -> Result<(), String> {
        if self.running.load(Ordering::SeqCst) {
            return Err("WASAPI loopback 已在运行".into());
        }
        self.running.store(true, Ordering::SeqCst);
        self.ring.lock().clear();

        let running = self.running.clone();
        let ring = self.ring.clone();
        let handle = std::thread::Builder::new()
            .name("wasapi-loopback".into())
            .spawn(move || {
                if let Err(e) = run_capture_loop(running.clone(), ring.clone()) {
                    tracing::error!("WASAPI loopback 采集线程退出：{}", e);
                }
                running.store(false, Ordering::SeqCst);
            })
            .map_err(|e| format!("启动采集线程失败：{}", e))?;
        *self.thread.lock() = Some(handle);
        Ok(())
    }

    fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(h) = self.thread.lock().take() {
            let _ = h.join();
        }
    }

    fn poll_frame(&mut self) -> Option<Vec<i16>> {
        let mut ring = self.ring.lock();
        if ring.len() < FRAME_SAMPLES_TOTAL {
            return None;
        }
        let mut frame = Vec::with_capacity(FRAME_SAMPLES_TOTAL);
        for _ in 0..FRAME_SAMPLES_TOTAL {
            frame.push(ring.pop_front().unwrap_or(0));
        }
        Some(frame)
    }

    fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

impl Drop for WasapiLoopbackCapture {
    fn drop(&mut self) {
        self.stop();
    }
}

/// 采集线程主循环。
fn run_capture_loop(
    running: Arc<AtomicBool>,
    ring: Arc<Mutex<VecDeque<i16>>>,
) -> Result<(), String> {
    use windows::Win32::Media::Audio::{
        eConsole, eRender, IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator,
        MMDeviceEnumerator, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK,
        WAVEFORMATEX, WAVEFORMATEXTENSIBLE,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
    };

    // WAVE 格式 tag 常量（windows crate 0.58 未直接导出）。
    const WAVE_FORMAT_IEEE_FLOAT: u16 = 3;
    const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;
    // AUDCLNT_BUFFERFLAGS_SILENT 标志位。
    const AUDCLNT_BUFFERFLAGS_SILENT: u32 = 0x2;

    // 1) COM 初始化（MTA）。S_OK / S_FALSE 都可接受。
    let co = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    let co_ok = co.is_ok();

    let result = (|| -> Result<(), String> {
        // 2) 枚举默认渲染端点。
        let enumerator: IMMDeviceEnumerator =
            unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
                .map_err(|e| format!("创建 IMMDeviceEnumerator 失败：{}", e))?;
        let device = unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eConsole) }
            .map_err(|e| format!("获取默认渲染端点失败：{}", e))?;

        // 3) 激活 IAudioClient。
        let audio_client: IAudioClient = unsafe { device.Activate(CLSCTX_ALL, None) }
            .map_err(|e| format!("Activate IAudioClient 失败：{}", e))?;

        // 4) 获取 mix format。
        let format_ptr = unsafe { audio_client.GetMixFormat() }
            .map_err(|e| format!("GetMixFormat 失败：{}", e))?;
        let format_ref = unsafe { &*format_ptr };

        // 解析格式：采样率、声道、tag。
        let src_rate = format_ref.nSamplesPerSec;
        let src_channels = format_ref.nChannels;
        let src_tag = format_ref.wFormatTag;
        let src_bits = format_ref.wBitsPerSample;

        // 判断是否为 float（WAVE_FORMAT_EXTENSIBLE 需查 subformat）。
        let is_float = if src_tag == WAVE_FORMAT_IEEE_FLOAT {
            true
        } else if src_tag == WAVE_FORMAT_EXTENSIBLE {
            // 尝试解释为 WAVEFORMATEXTENSIBLE（packed struct，需用 addr_of 读字段）。
            let ext_ptr = format_ptr as *const WAVEFORMATEX as *const WAVEFORMATEXTENSIBLE;
            let sub_format = unsafe { std::ptr::addr_of!((*ext_ptr).SubFormat).read_unaligned() };
            // KSDATAFORMAT_SUBTYPE_IEEE_FLOAT = {00000003-0000-0010-8000-00aa00389b71}
            sub_format == windows::core::GUID::from_u128(0x00000003_0000_0010_8000_00aa00389b71)
        } else {
            src_bits == 32
        };

        tracing::info!(
            "WASAPI loopback 格式：{}Hz {}ch {}bit tag={} float={}",
            src_rate,
            src_channels,
            src_bits,
            src_tag,
            is_float
        );

        // 5) 初始化（loopback 模式，100ms 缓冲）。
        let stream_flags = AUDCLNT_STREAMFLAGS_LOOPBACK;
        let hns_buffer: i64 = 1_000_000; // 100ms (100-nanosecond units)
        let init = unsafe {
            audio_client.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                stream_flags,
                hns_buffer,
                0,
                format_ref,
                None,
            )
        };
        init.map_err(|e| format!("IAudioClient Initialize 失败：{}", e))?;

        // 6) 获取 IAudioCaptureClient。
        let capture: IAudioCaptureClient =
            unsafe { audio_client.GetService::<IAudioCaptureClient>() }
                .map_err(|e| format!("GetService IAudioCaptureClient 失败：{}", e))?;

        // 7) Start。
        unsafe { audio_client.Start() }.map_err(|e| format!("IAudioClient Start 失败：{}", e))?;

        tracing::info!("WASAPI loopback 采集已启动。");

        // 8) 采集循环。
        let mut resample_state = ResampleState::new(src_rate, SAMPLE_RATE);
        let frame_needed = FRAME_SAMPLES_TOTAL; // 960 (stereo)
        let mut accum: Vec<i16> = Vec::with_capacity(frame_needed * 4);

        while running.load(Ordering::SeqCst) {
            // 等待一小段（~5ms）让缓冲有数据。
            std::thread::sleep(std::time::Duration::from_millis(5));

            // 处理所有可用包。
            loop {
                let packet_size = match unsafe { capture.GetNextPacketSize() } {
                    Ok(s) => s,
                    Err(_) => break,
                };
                if packet_size == 0 {
                    break;
                }

                let mut data_ptr: *mut u8 = std::ptr::null_mut();
                let mut frames_in_packet = 0u32;
                let mut flags = 0u32;

                let hr = unsafe {
                    capture.GetBuffer(
                        &mut data_ptr,
                        &mut frames_in_packet,
                        &mut flags,
                        None,
                        None,
                    )
                };
                if hr.is_err() {
                    break;
                }

                let silent = (flags & AUDCLNT_BUFFERFLAGS_SILENT) != 0;

                if !silent && !data_ptr.is_null() && frames_in_packet > 0 {
                    let samples_per_frame = src_channels as usize;
                    let total_samples = frames_in_packet as usize * samples_per_frame;

                    let converted: Vec<i16> = if is_float {
                        let float_ptr = data_ptr as *const f32;
                        let floats = unsafe { std::slice::from_raw_parts(float_ptr, total_samples) };
                        floats_to_i16(floats)
                    } else {
                        // 整数 PCM（少见）：按 bits 处理。
                        ints_to_i16(data_ptr, total_samples, src_bits)
                    };

                    // 声道归一化到 stereo。
                    let stereo = normalize_channels(&converted, src_channels as usize, 2);

                    // 重采样到 48kHz。
                    let resampled = resample_state.process(&stereo);

                    accum.extend(resampled);

                    // 每凑够 frame_needed 推一帧到 ring。
                    while accum.len() >= frame_needed {
                        let frame: Vec<i16> = accum.drain(..frame_needed).collect();
                        let mut rb = ring.lock();
                        // 防止 ring 无限增长：超过上限丢弃最旧。
                        let cap = CAPTURE_RING_FRAMES * FRAME_SAMPLES_TOTAL;
                        while rb.len() + frame.len() > cap {
                            rb.pop_front();
                        }
                        rb.extend(frame);
                    }
                }

                let _ = unsafe { capture.ReleaseBuffer(frames_in_packet) };
            }
        }

        // 9) Stop。
        let _ = unsafe { audio_client.Stop() };
        tracing::info!("WASAPI loopback 采集已停止。");
        Ok(())
    })();

    if co_ok {
        unsafe { CoUninitialize() };
    }
    result
}

/// f32 → i16（clamp）。
fn floats_to_i16(floats: &[f32]) -> Vec<i16> {
    floats
        .iter()
        .map(|&f| {
            let sample = f.clamp(-1.0, 1.0);
            if sample < 0.0 {
                (sample * 32768.0) as i16
            } else {
                (sample * 32767.0) as i16
            }
        })
        .collect()
}

/// 整数 PCM → i16。
fn ints_to_i16(data: *const u8, total_samples: usize, bits: u16) -> Vec<i16> {
    match bits {
        16 => {
            let ptr = data as *const i16;
            let slice = unsafe { std::slice::from_raw_parts(ptr, total_samples) };
            slice.to_vec()
        }
        24 => {
            let bytes = unsafe { std::slice::from_raw_parts(data, total_samples * 3) };
            bytes
                .chunks_exact(3)
                .map(|c| {
                    let lo = c[0] as i32;
                    let mid = c[1] as i32;
                    let hi = c[2] as i8 as i32;
                    ((hi << 16) | (mid << 8) | lo) >> 8
                })
                .map(|v| v as i16)
                .collect()
        }
        32 => {
            // 32-bit int。
            let ptr = data as *const i32;
            let slice = unsafe { std::slice::from_raw_parts(ptr, total_samples) };
            slice.iter().map(|&v| (v >> 16) as i16).collect()
        }
        _ => vec![0i16; total_samples],
    }
}

/// 声道数归一化（src_ch → dst_ch）。
fn normalize_channels(samples: &[i16], src_ch: usize, dst_ch: usize) -> Vec<i16> {
    if src_ch == dst_ch {
        return samples.to_vec();
    }
    let frames = samples.len() / src_ch.max(1);
    let mut out = Vec::with_capacity(frames * dst_ch);
    for f in 0..frames {
        let chs: Vec<i16> = (0..src_ch).map(|c| samples[f * src_ch + c]).collect();
        match (src_ch, dst_ch) {
            (1, 2) => {
                out.push(chs[0]);
                out.push(chs[0]);
            }
            (n, 2) if n >= 2 => {
                // 下混：L=ch0, R=ch1，其余忽略。
                out.push(chs[0]);
                out.push(chs[1]);
            }
            (_, 1) => {
                // 单声道下混：取均值。
                let sum: i32 = chs.iter().map(|&s| s as i32).sum();
                out.push((sum / chs.len() as i32) as i16);
            }
            _ => {
                // 兜底：前 dst_ch 路。
                for c in 0..dst_ch {
                    out.push(chs.get(c).copied().unwrap_or(0));
                }
            }
        }
    }
    out
}

/// 线性重采样状态（src_rate → dst_rate，立体声交错）。
struct ResampleState {
    src_rate: u32,
    dst_rate: u32,
    /// 上一帧末尾样本（立体声 = 2 值），用于插值起点。
    last: [i16; 2],
}

impl ResampleState {
    fn new(src_rate: u32, dst_rate: u32) -> Self {
        Self {
            src_rate,
            dst_rate,
            last: [0, 0],
        }
    }

    fn process(&mut self, input: &[i16]) -> Vec<i16> {
        if self.src_rate == self.dst_rate {
            return input.to_vec();
        }
        let ratio = self.dst_rate as f64 / self.src_rate as f64;
        let in_frames = input.len() / 2;
        let out_frames = ((in_frames as f64) * ratio).round() as usize;
        let mut out = Vec::with_capacity(out_frames * 2);
        for i in 0..out_frames {
            let src_pos = (i as f64) / ratio;
            let idx = src_pos.floor() as usize;
            let frac = src_pos - idx as f64;
            let (l0, r0) = if idx == 0 {
                (self.last[0], self.last[1])
            } else {
                let prev = (idx - 1) * 2;
                (input[prev], input[prev + 1])
            };
            let (l1, r1) = if idx < in_frames {
                let cur = idx * 2;
                (input[cur], input[cur + 1])
            } else {
                (self.last[0], self.last[1])
            };
            let l = (l0 as f64 + (l1 as f64 - l0 as f64) * frac) as i16;
            let r = (r0 as f64 + (r1 as f64 - r0 as f64) * frac) as i16;
            out.push(l);
            out.push(r);
        }
        if input.len() >= 2 {
            let n = input.len();
            self.last = [input[n - 2], input[n - 1]];
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floats_to_i16_clamps() {
        let r = floats_to_i16(&[1.0, -1.0, 0.5, 0.0]);
        assert_eq!(r, vec![32767, -32768, 16383, 0]);
    }

    #[test]
    fn normalize_mono_to_stereo() {
        let r = normalize_channels(&[100, 200, 300], 1, 2);
        assert_eq!(r, vec![100, 100, 200, 200, 300, 300]);
    }

    #[test]
    fn normalize_stereo_passthrough() {
        let r = normalize_channels(&[1, 2, 3, 4], 2, 2);
        assert_eq!(r, vec![1, 2, 3, 4]);
    }

    #[test]
    fn resample_passthrough_same_rate() {
        let mut rs = ResampleState::new(48000, 48000);
        let out = rs.process(&[1, 2, 3, 4]);
        assert_eq!(out, vec![1, 2, 3, 4]);
    }

    #[test]
    fn resample_downsample_48_to_44() {
        let mut rs = ResampleState::new(48000, 44100);
        // 480 frames → ~441 frames
        let input: Vec<i16> = (0..960).map(|i| i as i16).collect();
        let out = rs.process(&input);
        let out_frames = out.len() / 2;
        assert!(out_frames > 430 && out_frames < 450, "got {}", out_frames);
    }
}
