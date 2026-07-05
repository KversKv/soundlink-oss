//! 跨平台音频输出（cpal）。第一版统一用 cpal；WASAPI/CoreAudio 专用后端后续替换。
//!
//! 对齐 `docs/First/03-audio-pipeline.md` §2 接收端链路末端。
//! 提供：设备枚举、选择、低延迟播放（从 PlaybackSource 拉取 PCM i16）。

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{
    BufferSize, ChannelCount, Device, Host, SampleFormat, SampleRate, Stream, StreamConfig,
    SupportedStreamConfig,
};
use parking_lot::Mutex;
use std::sync::Arc;

/// 输出设备信息（供 UI 列表）。
#[derive(Debug, Clone)]
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
pub struct AudioOutput {
    host: Host,
    state: Mutex<OutputState>,
}

impl Default for AudioOutput {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioOutput {
    pub fn new() -> Self {
        Self {
            host: cpal::default_host(),
            state: Mutex::new(OutputState { stream: None }),
        }
    }

    /// 列举可用输出设备。
    pub fn list_devices(&self) -> Vec<OutputDeviceInfo> {
        let mut out = Vec::new();
        if let Ok(devs) = self.host.output_devices() {
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
        self.host.default_output_device()
    }

    /// 在指定设备上启动播放。`device_index` 为 None 时用默认设备。
    /// `source` 由 cpal 回调线程持续拉取。
    pub fn start(
        &self,
        device_index: Option<usize>,
        source: Box<dyn PlaybackSource>,
    ) -> Result<(), String> {
        let device = match device_index {
            Some(i) => self
                .host
                .output_devices()
                .map_err(|e| e.to_string())?
                .nth(i)
                .ok_or_else(|| format!("设备索引 {} 不存在", i))?,
            None => self
                .default_device()
                .ok_or_else(|| "无可用输出设备".to_string())?,
        };

        let supported = device
            .default_output_config()
            .map_err(|e| format!("获取默认输出配置失败：{}", e))?;
        let sample_format = supported.sample_format();
        // 用设备默认配置（cpal 不重采样）。PlaybackSource 供给交错 i16，按设备 channels 取用。
        let config: StreamConfig = supported.into();
        let channels = config.channels;

        let source: Arc<Mutex<Box<dyn PlaybackSource>>> = Arc::new(Mutex::new(source));
        let stream = match sample_format {
            SampleFormat::I16 => build_stream::<i16>(&device, &config, channels, source.clone()),
            SampleFormat::F32 => build_stream::<f32>(&device, &config, channels, source.clone()),
            SampleFormat::U16 => build_stream::<u16>(&device, &config, channels, source.clone()),
            other => return Err(format!("不支持的采样格式：{:?}", other)),
        }
        .map_err(|e| format!("构建输出流失败：{}", e))?;

        stream.play().map_err(|e| format!("启动播放失败：{}", e))?;
        self.state.lock().stream = Some(stream);
        Ok(())
    }

    /// 停止播放。
    pub fn stop(&self) {
        let stream = self.state.lock().stream.take();
        drop(stream);
    }
}

fn build_stream<T: cpal::SizedSample + FromSampleI16>(
    device: &Device,
    config: &StreamConfig,
    channels: ChannelCount,
    source: Arc<Mutex<Box<dyn PlaybackSource>>>,
) -> Result<Stream, cpal::BuildStreamError> {
    let mut cfg = config.clone();
    // 用默认缓冲（低延迟后续调优）。
    cfg.buffer_size = BufferSize::Default;
    let _ = channels;
    device.build_output_stream(
        &cfg,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            let mut tmp = vec![0i16; data.len()];
            {
                let mut s = source.lock();
                s.fill(&mut tmp);
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
    )
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
