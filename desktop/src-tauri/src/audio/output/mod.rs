//! 跨平台音频输出（cpal）。第一版统一用 cpal；WASAPI/CoreAudio 专用后端后续替换。
//!
//! 对齐 `docs/First/03-audio-pipeline.md` §2 接收端链路末端。
//! 提供：设备枚举、选择、低延迟播放（从 PlaybackSource 拉取 PCM i16）。

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{
    BufferSize, ChannelCount, Device, SampleFormat, SampleRate, Stream, StreamConfig,
    SupportedStreamConfig,
};
use std::cell::RefCell;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use crate::constants::OUTPUT_BUFFER_SAMPLES;

/// 软件音量共享状态：用 AtomicU32 存储 f32::to_bits，避免回调里加锁。
/// 取值范围 [0.0, 1.0]，1.0 = 不增不减。
#[derive(Clone)]
pub struct VolumeControl(Arc<AtomicU32>);

impl VolumeControl {
    pub fn new() -> Self {
        Self(Arc::new(AtomicU32::new(1.0f32.to_bits())))
    }

    pub fn set(&self, v: f32) {
        let clamped = v.clamp(0.0, 1.0);
        self.0.store(clamped.to_bits(), Ordering::Relaxed);
    }

    pub fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }
}

impl Default for VolumeControl {
    fn default() -> Self {
        Self::new()
    }
}

/// 输出设备信息（供 UI 列表）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct OutputDeviceInfo {
    pub id: String,
    pub name: String,
}

/// 播放源：cpal 回调从此拉取交错 i16 PCM。
pub trait PlaybackSource: Send + 'static {
    /// 用下一批样本填充 `out`（长度 = frames * channels）。欠流时填静音。
    fn fill(&mut self, out: &mut [i16]);
}

struct OutputState {
    stream: Option<Stream>,
}

/// 音频输出器。持有 stream 保持播放。
///
/// 注意：cpal 0.15 的 `Stream` 是 `Send`，`play()`/`pause()`/`drop()`
/// 可跨线程调用，因此不再强制 start/stop 在创建线程上执行。
pub struct AudioOutput {
    state: RefCell<OutputState>,
    /// 软件音量控制（运行时可在回调外调整）。
    volume: VolumeControl,
}

impl Default for AudioOutput {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioOutput {
    pub fn new() -> Self {
        Self {
            state: RefCell::new(OutputState { stream: None }),
            volume: VolumeControl::new(),
        }
    }

    /// 设置软件音量 `v ∈ [0.0, 1.0]`。运行时实时生效（下一帧回调即应用）。
    pub fn set_volume(&self, v: f32) {
        self.volume.set(v);
    }

    /// 当前音量。
    pub fn volume(&self) -> f32 {
        self.volume.get()
    }

    /// 列举可用输出设备。
    pub fn list_devices(&self) -> Vec<OutputDeviceInfo> {
        let host = cpal::default_host();
        let mut out = Vec::new();
        if let Ok(devs) = host.output_devices() {
            for (i, d) in devs.enumerate() {
                let name = d.name().unwrap_or_else(|_| format!("Device {}", i));
                out.push(OutputDeviceInfo {
                    id: i.to_string(),
                    name,
                });
            }
        }
        out
    }

    /// 默认输出设备。
    pub fn default_device(&self) -> Option<Device> {
        cpal::default_host().default_output_device()
    }

    /// 在指定设备上启动播放。`device_index` 为 None 时用默认设备。
    /// `source` 由 cpal 回调线程持续拉取。
    pub fn start(
        &self,
        device_index: Option<usize>,
        source: Box<dyn PlaybackSource>,
    ) -> Result<(), String> {
        let host = cpal::default_host();
        let device = match device_index {
            Some(i) => host
                .output_devices()
                .map_err(|e| e.to_string())?
                .nth(i)
                .ok_or_else(|| format!("设备索引 {} 不存在", i))?,
            None => host
                .default_output_device()
                .ok_or_else(|| "无可用输出设备".to_string())?,
        };

        let supported = device
            .default_output_config()
            .map_err(|e| format!("获取默认输出配置失败：{}", e))?;
        let sample_format = supported.sample_format();
        // 用设备默认配置（cpal 不重采样）。PlaybackSource 供给交错 i16，按设备 channels 取用。
        let config: StreamConfig = supported.into();
        let channels = config.channels;

        let source: Arc<parking_lot::Mutex<Box<dyn PlaybackSource>>> =
            Arc::new(parking_lot::Mutex::new(source));
        let volume = self.volume.clone();
        let stream = match sample_format {
            SampleFormat::I16 => {
                build_stream::<i16>(&device, &config, channels, source.clone(), volume.clone())
            }
            SampleFormat::F32 => {
                build_stream::<f32>(&device, &config, channels, source.clone(), volume.clone())
            }
            SampleFormat::U16 => {
                build_stream::<u16>(&device, &config, channels, source.clone(), volume.clone())
            }
            other => return Err(format!("不支持的采样格式：{:?}", other)),
        }
        .map_err(|e| format!("构建输出流失败：{}", e))?;

        stream.play().map_err(|e| format!("启动播放失败：{}", e))?;
        self.state.borrow_mut().stream = Some(stream);
        Ok(())
    }

    /// 停止播放。
    pub fn stop(&self) {
        let stream = self.state.borrow_mut().stream.take();
        drop(stream);
    }
}

unsafe impl Send for AudioOutput {}
unsafe impl Sync for AudioOutput {}

fn build_stream<T: cpal::SizedSample + FromSampleI16>(
    device: &Device,
    config: &StreamConfig,
    channels: ChannelCount,
    source: Arc<parking_lot::Mutex<Box<dyn PlaybackSource>>>,
    volume: VolumeControl,
) -> Result<Stream, cpal::BuildStreamError> {
    let mut cfg = config.clone();
    let _ = channels;
    // 阶段 4：低延迟 buffer 调优。优先用固定 buffer（OUTPUT_BUFFER_SAMPLES），
    // 失败则回退默认。
    cfg.buffer_size = BufferSize::Fixed(OUTPUT_BUFFER_SAMPLES);
    let source_for_fixed = source.clone();
    let vol_for_fixed = volume.clone();
    let stream_result = device.build_output_stream(
        &cfg,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            let mut tmp = vec![0i16; data.len()];
            {
                let mut s = source_for_fixed.lock();
                s.fill(&mut tmp);
            }
            // 应用软件音量（i16 阶段，避免每个采样类型重复算）。
            let vol = vol_for_fixed.get();
            if vol != 1.0 {
                for s in tmp.iter_mut() {
                    let scaled = (*s as f32 * vol).clamp(-32768.0, 32767.0) as i16;
                    *s = scaled;
                }
            }
            // 若设备声道数与源（2）不同，做简单截断/复制。
            let ch = cfg.channels as usize;
            if ch == 2 || ch == 0 {
                for (dst, src) in data.iter_mut().zip(tmp.iter()) {
                    *dst = T::from_i16(*src);
                }
            } else {
                // 按帧复制 stereo→多声道（前两路用源，其余静音）或下混。
                let frames = data.len() / ch;
                for f in 0..frames {
                    let l = tmp.get(f * 2).copied().unwrap_or(0);
                    let r = tmp.get(f * 2 + 1).copied().unwrap_or(l);
                    for c in 0..ch {
                        let v = if c == 0 {
                            l
                        } else if c == 1 {
                            r
                        } else {
                            (l + r) / 2
                        };
                        data[f * ch + c] = T::from_i16(v);
                    }
                }
            }
        },
        |err| tracing::error!("cpal 输出错误：{}", err),
        None,
    );
    match stream_result {
        Ok(s) => Ok(s),
        Err(_) => {
            // 回退默认 buffer。
            cfg.buffer_size = BufferSize::Default;
            tracing::warn!("Fixed buffer size 不支持，回退 Default");
            let vol_for_default = volume.clone();
            device.build_output_stream(
                &cfg,
                move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                    let mut tmp = vec![0i16; data.len()];
                    {
                        let mut s = source.lock();
                        s.fill(&mut tmp);
                    }
                    let vol = vol_for_default.get();
                    if vol != 1.0 {
                        for s in tmp.iter_mut() {
                            let scaled = (*s as f32 * vol).clamp(-32768.0, 32767.0) as i16;
                            *s = scaled;
                        }
                    }
                    let ch = cfg.channels as usize;
                    if ch == 2 || ch == 0 {
                        for (dst, src) in data.iter_mut().zip(tmp.iter()) {
                            *dst = T::from_i16(*src);
                        }
                    } else {
                        let frames = data.len() / ch;
                        for f in 0..frames {
                            let l = tmp.get(f * 2).copied().unwrap_or(0);
                            let r = tmp.get(f * 2 + 1).copied().unwrap_or(l);
                            for c in 0..ch {
                                let v = if c == 0 {
                                    l
                                } else if c == 1 {
                                    r
                                } else {
                                    (l + r) / 2
                                };
                                data[f * ch + c] = T::from_i16(v);
                            }
                        }
                    }
                },
                |err| tracing::error!("cpal 输出错误（默认 buffer）：{}", err),
                None,
            )
        }
    }
}

/// `FromSampleI16` 帮 cpal 各采样类型从 i16 转换。
pub trait FromSampleI16: cpal::Sample {
    fn from_i16(s: i16) -> Self;
}
impl FromSampleI16 for i16 {
    fn from_i16(s: i16) -> Self {
        s
    }
}
impl FromSampleI16 for f32 {
    fn from_i16(s: i16) -> Self {
        s as f32 / 32768.0
    }
}
impl FromSampleI16 for u16 {
    fn from_i16(s: i16) -> Self {
        (s as i32 + 32768) as u16
    }
}

/// 强制 48kHz/Stereo 的输出配置（若设备支持）。
#[allow(dead_code)]
fn try_forced_config(device: &Device) -> Option<SupportedStreamConfig> {
    let mut supported: Option<SupportedStreamConfig> = None;
    if let Ok(configs) = device.supported_output_configs() {
        for c in configs {
            if c.channels() == 2 && c.min_sample_rate().0 <= 48000 && c.max_sample_rate().0 >= 48000
            {
                supported = Some(c.with_sample_rate(SampleRate(48000)));
                break;
            }
        }
    }
    supported
}
