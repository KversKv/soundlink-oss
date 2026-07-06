//! 接收端引擎：UDP 收包 → AEAD 解密 → Jitter Buffer → Opus 解码(PLC) → 漂移校正 → cpal 输出。
//!
//! 阶段 4 增强（对齐 `docs/First/11-implementation-spec.md` §7、`03-audio-pipeline.md`）：
//! - 自适应 Jitter Buffer（Low/Balanced/Stable/Auto 模式，可运行时切换）。
//! - Opus PLC 连续帧上限（超过切静音，避免衰减 artifacts）。
//! - 时钟漂移校正（DriftResampler，±0.5% 线性重采样）。
//! - 延迟估算（基于 sender timestamp 与本地时钟差）。
//! - 抖动/丢包率/码率统计（供 UI 与 stats 回传）。
//!
//! 同时被 Tauri commands（应用）与 `examples/*`（自测）使用，不依赖 Tauri。

use crate::audio::jitter_buffer::{JitterBuffer, JitterFrame, JitterMode, PopResult};
use crate::audio::opus_codec::{default_codec, frame_pcm_len, AudioCodec};
use crate::audio::output::{AudioOutput, PlaybackSource};
use crate::audio::resampler::DriftResampler;
use crate::constants::{
    BITRATE_MAX, BITRATE_MIN, BITRATE_STEP, DEFAULT_AUDIO_PORT, DEFAULT_JITTER_MS,
    EST_LATENCY_INIT_MS, FRAME_DURATION_MS, LOSS_RATE_HIGH_THRESHOLD,
    LOSS_RATE_LOW_THRESHOLD, OUTPUT_BUFFER_FRAMES, PLC_CONSECUTIVE_LIMIT, SAMPLE_RATE,
};
use crate::network::packet::decode_packet;
use parking_lot::Mutex;
use serde::Serialize;
use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::Write;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::net::UdpSocket;
use tokio::task::JoinHandle;

/// 调试保存器：把接收链路各阶段数据落盘，便于诊断杂音/错位问题。
///
/// 启用条件（任一成立）：
/// - `dump_enable = true`（来自 main.rs 的 `DUMP_ENABLE`）
/// - 环境变量 `SOUNDLINK_DUMP=1`（兼容旧用法）
///
/// 保存三类文件（覆盖写）：
/// - `soundlink_opus.bin`：原始 Opus 帧，每帧前 4 字节小端长度前缀
/// - `soundlink_pcm_decoded.raw`：Opus 解码后 PCM（i16 LE，stereo 交错）
/// - `soundlink_pcm_resampled.raw`：漂移校正后 PCM（i16 LE，stereo 交错，送 cpal 前）
///
/// 用 Audacity / ffmpeg / Python 可直接分析：
///   ffmpeg -f s16le -ar 48000 -ac 2 -i soundlink_pcm_decoded.raw out.wav
///
/// 实现：mpsc 异步队列。音频回调线程只负责 send（非阻塞），独立 IO 线程负责写文件，
/// 避免阻塞 cpal 实时回调（否则会导致音频卡顿甚至 WASAPI 强制停流）。
struct DebugDumper {
    tx: std::sync::mpsc::Sender<DumpMsg>,
    _io_thread: std::thread::JoinHandle<()>,
}

/// 转储消息（音频线程 → IO 线程）。
enum DumpMsg {
    Opus { data: Vec<u8>, seq: u32, lost: bool },
    PcmDecoded(Vec<i16>),
    PcmResampled(Vec<i16>),
    /// 通知 IO 线程刷新并关闭。
    Shutdown,
}

impl DebugDumper {
    fn new(dump_enable: bool) -> Option<Self> {
        let env_on = std::env::var("SOUNDLINK_DUMP").ok().as_deref() == Some("1");
        if !dump_enable && !env_on {
            return None;
        }
        let opus_file = OpenOptions::new()
            .create(true).truncate(true).write(true)
            .open("soundlink_opus.bin").ok()?;
        let pcm_decoded_file = OpenOptions::new()
            .create(true).truncate(true).write(true)
            .open("soundlink_pcm_decoded.raw").ok()?;
        let pcm_resampled_file = OpenOptions::new()
            .create(true).truncate(true).write(true)
            .open("soundlink_pcm_resampled.raw").ok()?;
        let (tx, rx) = std::sync::mpsc::channel::<DumpMsg>();
        tracing::info!(
            "调试保存已启用：soundlink_opus.bin / soundlink_pcm_decoded.raw / soundlink_pcm_resampled.raw"
        );
        // IO 线程：从队列取消息写文件，避免阻塞音频回调。
        let io_thread = std::thread::Builder::new()
            .name("soundlink-dump-io".into())
            .spawn(move || {
                let mut opus_file = opus_file;
                let mut pcm_decoded_file = pcm_decoded_file;
                let mut pcm_resampled_file = pcm_resampled_file;
                for msg in rx.iter() {
                    match msg {
                        DumpMsg::Opus { data, seq, lost } => {
                            let marker: u32 = if lost { 0xFFFF_FFFF } else { data.len() as u32 };
                            let _ = opus_file.write_all(&marker.to_le_bytes());
                            let _ = opus_file.write_all(&seq.to_le_bytes());
                            if !lost {
                                let _ = opus_file.write_all(&data);
                            }
                        }
                        DumpMsg::PcmDecoded(pcm) => {
                            let mut bytes = Vec::with_capacity(pcm.len() * 2);
                            for &s in &pcm {
                                bytes.extend_from_slice(&s.to_le_bytes());
                            }
                            let _ = pcm_decoded_file.write_all(&bytes);
                        }
                        DumpMsg::PcmResampled(pcm) => {
                            let mut bytes = Vec::with_capacity(pcm.len() * 2);
                            for &s in &pcm {
                                bytes.extend_from_slice(&s.to_le_bytes());
                            }
                            let _ = pcm_resampled_file.write_all(&bytes);
                        }
                        DumpMsg::Shutdown => break,
                    }
                }
                // 排空队列剩余消息（避免 Drop 时丢失未处理数据）。
                for msg in rx.try_iter() {
                    match msg {
                        DumpMsg::Opus { data, seq, lost } => {
                            let marker: u32 = if lost { 0xFFFF_FFFF } else { data.len() as u32 };
                            let _ = opus_file.write_all(&marker.to_le_bytes());
                            let _ = opus_file.write_all(&seq.to_le_bytes());
                            if !lost {
                                let _ = opus_file.write_all(&data);
                            }
                        }
                        DumpMsg::PcmDecoded(pcm) => {
                            let mut bytes = Vec::with_capacity(pcm.len() * 2);
                            for &s in &pcm {
                                bytes.extend_from_slice(&s.to_le_bytes());
                            }
                            let _ = pcm_decoded_file.write_all(&bytes);
                        }
                        DumpMsg::PcmResampled(pcm) => {
                            let mut bytes = Vec::with_capacity(pcm.len() * 2);
                            for &s in &pcm {
                                bytes.extend_from_slice(&s.to_le_bytes());
                            }
                            let _ = pcm_resampled_file.write_all(&bytes);
                        }
                        DumpMsg::Shutdown => {}
                    }
                }
                tracing::info!("调试保存 IO 线程退出");
            })
            .ok()?;
        Some(Self { tx, _io_thread: io_thread })
    }

    /// 保存原始 Opus 帧（4 字节小端长度前缀 + 数据）。
    fn dump_opus(&self, data: &[u8], seq: u32, lost: bool) {
        let _ = self.tx.send(DumpMsg::Opus {
            data: data.to_vec(),
            seq,
            lost,
        });
    }

    /// 保存解码后 PCM（i16 LE）。
    fn dump_pcm_decoded(&self, pcm: &[i16]) {
        let _ = self.tx.send(DumpMsg::PcmDecoded(pcm.to_vec()));
    }

    /// 保存重采样后 PCM（i16 LE）。
    fn dump_pcm_resampled(&self, pcm: &[i16]) {
        let _ = self.tx.send(DumpMsg::PcmResampled(pcm.to_vec()));
    }
}

impl Drop for DebugDumper {
    fn drop(&mut self) {
        let _ = self.tx.send(DumpMsg::Shutdown);
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ReceiverStatus {
    pub state: String, // "IDLE" | "RECEIVING" | "ERROR"
    pub packets_recv: u64,
    pub packets_lost: u64,
    pub packets_dropped: u64,
    pub buffer_depth: usize,
    pub buffer_ms: u32,
    pub est_latency_ms: u32,
    /// 网络抖动 EWMA（ms）。
    pub jitter_ms: u32,
    /// 丢包率（0..1）。
    pub loss_rate: f64,
    /// 接收码率（bps，从 payload 实测）。
    pub bitrate: u32,
    /// 当前 Jitter 模式。
    pub jitter_mode: String,
    /// 给 sender 的码率建议（bps）。
    pub recommended_bitrate: u32,
    /// 当前漂移校正比率（1.0 = 无校正）。
    pub drift_ratio: f64,
    /// 连续 PLC 帧数（调试）。
    pub consecutive_plc: usize,
}

impl Default for ReceiverStatus {
    fn default() -> Self {
        Self {
            state: "IDLE".into(),
            packets_recv: 0,
            packets_lost: 0,
            packets_dropped: 0,
            buffer_depth: 0,
            buffer_ms: 0,
            est_latency_ms: EST_LATENCY_INIT_MS,
            jitter_ms: 0,
            loss_rate: 0.0,
            bitrate: 0,
            jitter_mode: JitterMode::from_ms(DEFAULT_JITTER_MS).as_str().to_string(),
            recommended_bitrate: 0,
            drift_ratio: 1.0,
            consecutive_plc: 0,
        }
    }
}

/// 接收端引擎。
pub struct ReceiverEngine {
    status: Arc<Mutex<ReceiverStatus>>,
    jitter: Arc<Mutex<JitterBuffer>>,
    codec: Arc<Mutex<Box<dyn AudioCodec>>>,
    running: Arc<AtomicBool>,
    udp_task: Mutex<Option<JoinHandle<()>>>,
    audio_output: Mutex<AudioOutput>,
    /// 延迟估算共享状态（push 端写入，status 端读取）。
    latency_state: Arc<Mutex<LatencyState>>,
    /// 是否启用音频 RAW Data 转储（来自 main.rs 的 DUMP_ENABLE）。
    dump_enable: bool,
}

/// 延迟估算状态：记录首个包接收时刻与 timestamp 基准。
#[derive(Debug, Default)]
struct LatencyState {
    first_recv_instant: Option<Instant>,
    first_timestamp: u64,
    latest_timestamp: u64,
    /// 累计接收 payload 字节数（用于码率计算）。
    bytes_recv: u64,
    /// 码率计算起点。
    bitrate_start: Option<Instant>,
    /// 上次码率计算时刻的累计字节。
    bitrate_baseline_bytes: u64,
    /// 上次码率（bps）。
    last_bitrate: u32,
    /// 当前漂移校正比率（由播放线程更新）。
    last_drift_ratio: f64,
    /// 连续 PLC 帧数（由播放线程更新，供 status 读取）。
    consecutive_plc: usize,
}

impl ReceiverEngine {
    pub fn new() -> Self {
        Self::with_dump(false)
    }

    /// `dump_enable = true` 时启用音频各阶段 RAW Data 转储。
    /// 仍可用环境变量 `SOUNDLINK_DUMP=1` 强制开启（兼容旧用法）。
    pub fn with_dump(dump_enable: bool) -> Self {
        Self {
            status: Arc::new(Mutex::new(ReceiverStatus::default())),
            jitter: Arc::new(Mutex::new(JitterBuffer::new(DEFAULT_JITTER_MS))),
            codec: Arc::new(Mutex::new(default_codec())),
            running: Arc::new(AtomicBool::new(false)),
            udp_task: Mutex::new(None),
            audio_output: Mutex::new(AudioOutput::new()),
            latency_state: Arc::new(Mutex::new(LatencyState::default())),
            dump_enable,
        }
    }

    /// 启动接收：绑定 UDP、起 cpal 输出。
    pub async fn start(
        &self,
        audio_key: [u8; 32],
        stream_id: u32,
        bind_addr: &str,
        device_index: Option<usize>,
    ) -> Result<(), String> {
        if self.running.load(Ordering::SeqCst) {
            return Err("接收器已在运行".into());
        }
        let sock = UdpSocket::bind(bind_addr)
            .await
            .map_err(|e| format!("绑定 {} 失败：{}", bind_addr, e))?;
        // 重置 jitter/状态。
        self.jitter.lock().reset();
        {
            let mut s = self.status.lock();
            let mode = s.jitter_mode.clone();
            *s = ReceiverStatus::default();
            s.jitter_mode = mode;
        }
        self.latency_state.lock().first_recv_instant = None;
        self.latency_state.lock().bitrate_start = None;
        self.running.store(true, Ordering::SeqCst);

        // cpal 输出。
        let playback = Box::new(PlaybackFromJitter::new(
            self.jitter.clone(),
            self.codec.clone(),
            self.latency_state.clone(),
            self.dump_enable,
        ));
        if let Err(e) = self.audio_output.lock().start(device_index, playback) {
            tracing::warn!("cpal 输出启动失败（继续收包但不发声）：{}", e);
        }

        // UDP 收包任务。
        let status = self.status.clone();
        let jitter = self.jitter.clone();
        let latency_state = self.latency_state.clone();
        let running = self.running.clone();
        let handle = tokio::spawn(async move {
            let mut buf = vec![0u8; 4096];
            let mut first_pkt = true;
            while running.load(Ordering::SeqCst) {
                match sock.recv_from(&mut buf).await {
                    Ok((n, _src)) => match decode_packet(&audio_key, &buf[..n]) {
                        Ok(dec) => {
                            if dec.header.stream_id != stream_id {
                                continue;
                            }
                            let payload_len = dec.plaintext.len();
                            let frame = JitterFrame {
                                sequence: dec.header.sequence,
                                timestamp: dec.header.timestamp,
                                data: dec.plaintext,
                            };
                            // 延迟估算：首个包记录基准。
                            {
                                let mut ls = latency_state.lock();
                                if ls.first_recv_instant.is_none() {
                                    ls.first_recv_instant = Some(Instant::now());
                                    ls.first_timestamp = frame.timestamp;
                                    ls.bitrate_start = Some(Instant::now());
                                    ls.bitrate_baseline_bytes = 0;
                                }
                                ls.latest_timestamp = frame.timestamp;
                                ls.bytes_recv += payload_len as u64;
                            }
                            let stats = {
                                let mut jb = jitter.lock();
                                jb.push(frame);
                                snapshot(&jb)
                            };
                            {
                                let mut s = status.lock();
                                if first_pkt {
                                    s.state = "RECEIVING".into();
                                    first_pkt = false;
                                }
                                s.packets_recv = stats.recv;
                                s.packets_lost = stats.lost;
                                s.packets_dropped = stats.dropped;
                                s.buffer_depth = stats.depth;
                                s.buffer_ms = (stats.depth as u32) * FRAME_DURATION_MS as u32;
                                s.jitter_ms = stats.jitter_ms;
                                s.jitter_mode = stats.mode;
                            }
                        }
                        Err(e) => {
                            tracing::debug!("收包解密失败，丢弃：{:?}", e);
                        }
                    },
                    Err(e) => {
                        tracing::warn!("UDP recv_from 错误：{}", e);
                        break;
                    }
                }
            }
        });
        *self.udp_task.lock() = Some(handle);
        Ok(())
    }

    /// 停止接收。
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(h) = self.udp_task.lock().take() {
            h.abort();
        }
        self.audio_output.lock().stop();
        let mut s = self.status.lock();
        s.state = "IDLE".into();
        s.buffer_depth = 0;
        s.buffer_ms = 0;
        s.consecutive_plc = 0;
    }

    /// 状态快照（含实时刷新丢包/缓冲/延迟/码率）。
    pub fn status(&self) -> ReceiverStatus {
        // 先从 latency_state 提取需要写回 latency_state 的值（避免嵌套锁）。
        let mut bitrate_to_write: Option<(Instant, u64, u32)> = None;
        {
            let ls = self.latency_state.lock();
            let mut s = self.status.lock();
            s.drift_ratio = ls.last_drift_ratio;
            s.consecutive_plc = ls.consecutive_plc;
            if let (Some(start), Some(bps_start)) = (ls.first_recv_instant, ls.bitrate_start) {
                // 延迟 = 接收端 wall-clock 经过时间 - sender 时钟经过时间（采样计数换算）
                // + 输出缓冲（OUTPUT_BUFFER_FRAMES * 10ms）。
                let wall_ms = start.elapsed().as_millis() as u64;
                let ts_diff_samples = ls.latest_timestamp.saturating_sub(ls.first_timestamp);
                let sender_ms = ts_diff_samples / (SAMPLE_RATE as u64 / 1000);
                let output_buffer_ms = OUTPUT_BUFFER_FRAMES as u64 * FRAME_DURATION_MS as u64;
                let est = wall_ms.saturating_sub(sender_ms).saturating_add(output_buffer_ms);
                s.est_latency_ms = est as u32;

                // 码率：每秒刷新一次。
                let elapsed = bps_start.elapsed().as_secs_f64();
                if elapsed >= 1.0 {
                    let bytes_delta = ls.bytes_recv.saturating_sub(ls.bitrate_baseline_bytes);
                    let bps = (bytes_delta as f64 * 8.0 / elapsed) as u32;
                    s.bitrate = bps;
                    bitrate_to_write = Some((Instant::now(), ls.bytes_recv, bps));
                } else {
                    s.bitrate = ls.last_bitrate;
                }
            }
        }
        // 写回码率基线（已释放所有锁）。
        if let Some((new_start, new_baseline, bps)) = bitrate_to_write {
            let mut ls = self.latency_state.lock();
            ls.bitrate_start = Some(new_start);
            ls.bitrate_baseline_bytes = new_baseline;
            ls.last_bitrate = bps;
        }
        // 再刷新 jitter 统计（独立锁，避免与 latency_state 嵌套）。
        {
            let jb = self.jitter.lock();
            let mut s = self.status.lock();
            s.packets_recv = jb.packets_recv;
            s.packets_lost = jb.packets_lost;
            s.packets_dropped = jb.packets_dropped;
            s.buffer_depth = jb.depth();
            s.buffer_ms = (jb.depth() as u32) * FRAME_DURATION_MS as u32;
            s.jitter_ms = jb.jitter_ms();
            s.jitter_mode = jb.mode().as_str().to_string();
            // 丢包率。
            let total = s.packets_recv + s.packets_lost;
            s.loss_rate = if total > 0 {
                s.packets_lost as f64 / total as f64
            } else {
                0.0
            };
            // 码率建议（根据丢包率）。
            s.recommended_bitrate = recommend_bitrate(s.loss_rate, s.packets_recv);
        }
        self.status.lock().clone()
    }

    /// 切换 Jitter 模式（运行时）。
    pub fn set_jitter_mode(&self, mode: JitterMode) {
        self.jitter.lock().switch_mode(mode);
        let mut s = self.status.lock();
        s.jitter_mode = mode.as_str().to_string();
        tracing::info!("Jitter 模式切换：{}", mode.as_str());
    }

    /// 当前 Jitter 模式。
    pub fn jitter_mode(&self) -> JitterMode {
        self.jitter.lock().mode()
    }

    /// 设置软件音量 `v ∈ [0.0, 1.0]`。运行时实时生效（不需要重启流）。
    pub fn set_volume(&self, v: f32) {
        self.audio_output.lock().set_volume(v);
        tracing::info!("输出音量设置为 {:.0}%", (v * 100.0).round());
    }

    /// 当前音量 `∈ [0.0, 1.0]`。
    pub fn volume(&self) -> f32 {
        self.audio_output.lock().volume()
    }

    /// 是否正在接收。
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

impl Default for ReceiverEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ReceiverEngine {
    fn drop(&mut self) {
        self.stop();
    }
}

/// 从 Jitter Buffer + Opus 解码器 + 漂移校正拉取 PCM 的播放源。
struct PlaybackFromJitter {
    jitter: Arc<Mutex<JitterBuffer>>,
    codec: Arc<Mutex<Box<dyn AudioCodec>>>,
    latency_state: Arc<Mutex<LatencyState>>,
    residue: VecDeque<i16>,
    resampler: DriftResampler,
    /// 连续 PLC 计数（用于限制）。
    consecutive_plc: usize,
    /// 临时缓冲：重采样后可能多于/少于一帧，需跨调用累积。
    resampled: VecDeque<i16>,
    /// 调试保存器（None = 未启用）。
    dumper: Option<DebugDumper>,
}

impl PlaybackFromJitter {
    fn new(
        jitter: Arc<Mutex<JitterBuffer>>,
        codec: Arc<Mutex<Box<dyn AudioCodec>>>,
        latency_state: Arc<Mutex<LatencyState>>,
        dump_enable: bool,
    ) -> Self {
        Self {
            jitter,
            codec,
            latency_state,
            residue: VecDeque::with_capacity(frame_pcm_len() * 4),
            resampler: DriftResampler::new(),
            consecutive_plc: 0,
            resampled: VecDeque::with_capacity(frame_pcm_len() * 4),
            dumper: DebugDumper::new(dump_enable),
        }
    }

    /// 解码一帧 → 重采样 → 推入 resampled。
    fn produce_one_frame(&mut self) {
        let (pop_result, depth, target) = {
            let mut jb = self.jitter.lock();
            let r = jb.pop();
            let d = jb.depth();
            let t = jb.target_depth();
            (r, d, t)
        };
        // 根据缓冲水位更新漂移校正比率。
        self.resampler.observe(depth, target);

        let pcm: Vec<i16> = match pop_result {
            PopResult::Frame(ref f) => {
                self.consecutive_plc = 0;
                let decoded = self.codec.lock().decode(&f.data);
                if let Some(d) = self.dumper.as_ref() {
                    d.dump_opus(&f.data, f.sequence, false);
                    d.dump_pcm_decoded(&decoded);
                }
                decoded
            }
            PopResult::Lost => {
                self.consecutive_plc += 1;
                if let Some(d) = self.dumper.as_ref() {
                    // 用当前 played_watermark 推断丢失 seq 不准；用 0 占位。
                    d.dump_opus(&[], 0, true);
                }
                if self.consecutive_plc > PLC_CONSECUTIVE_LIMIT {
                    // 超过连续 PLC 上限：切静音，避免 Opus PLC 持续衰减 artifacts。
                    vec![0i16; frame_pcm_len()]
                } else {
                    self.codec.lock().decode_plc()
                }
            }
            PopResult::Empty => {
                // 欠流：缓冲耗尽，直接返回静音。
                // 注意：不调用 decode_plc()——PLC 会推进 Opus 解码器内部状态，
                // 当真实帧到达时解码器状态已偏移，导致后续解码输出噪声。
                // PLC 仅用于真正丢包（Lost），欠流用静音更安全。
                self.consecutive_plc = self.consecutive_plc.saturating_add(1);
                vec![0i16; frame_pcm_len()]
            }
        };

        // 重采样并累积到 resampled。
        let out = self.resampler.process(&pcm);
        if let Some(d) = self.dumper.as_ref() {
            d.dump_pcm_resampled(&out);
        }
        self.resampled.extend(out);

        // 更新漂移比率与连续 PLC 计数到共享状态（供 status() 读取）。
        {
            let mut ls = self.latency_state.lock();
            ls.last_drift_ratio = self.resampler.ratio();
            ls.consecutive_plc = self.consecutive_plc;
        }
    }
}

impl PlaybackSource for PlaybackFromJitter {
    fn fill(&mut self, out: &mut [i16]) {
        // 持续生产帧直到 resampled 足够填充 out。
        while self.resampled.len() < out.len() {
            self.produce_one_frame();
        }
        for dst in out.iter_mut() {
            *dst = self.resampled.pop_front().unwrap_or(0);
        }
        // residue 在重采样路径下不再使用，保留字段以兼容旧测试。
        let _ = &mut self.residue;
    }
}

/// 根据丢包率计算建议码率（bps）。
/// - loss_rate > 5%：下调（每次 -16kbps，下限 32kbps）。
/// - loss_rate < 1%：上调（每次 +16kbps，上限 192kbps）。
/// - 中间：维持 128kbps。
fn recommend_bitrate(loss_rate: f64, packets_recv: u64) -> u32 {
    if packets_recv < 50 {
        return 0; // 样本不足，不建议。
    }
    let baseline: u32 = 128_000;
    if loss_rate > LOSS_RATE_HIGH_THRESHOLD {
        // 高丢包：下调。每超 5% 多降一档。
        let extra = ((loss_rate - LOSS_RATE_HIGH_THRESHOLD) / 0.05) as u32;
        let step = (1 + extra) * BITRATE_STEP;
        baseline.saturating_sub(step).max(BITRATE_MIN)
    } else if loss_rate < LOSS_RATE_LOW_THRESHOLD {
        // 低丢包：上调。
        baseline + BITRATE_STEP.min(BITRATE_MAX - baseline)
    } else {
        baseline
    }
}

/// 默认绑定地址（127.0.0.1 用于自测；实际可用 0.0.0.0 接收外部）。
pub fn default_bind_addr() -> String {
    format!("0.0.0.0:{}", DEFAULT_AUDIO_PORT)
}

/// Jitter 统计快照（避免同时持有 jitter 与 status 锁）。
struct JitterStats {
    recv: u64,
    lost: u64,
    dropped: u64,
    depth: usize,
    jitter_ms: u32,
    mode: String,
}
fn snapshot(jb: &JitterBuffer) -> JitterStats {
    JitterStats {
        recv: jb.packets_recv,
        lost: jb.packets_lost,
        dropped: jb.packets_dropped,
        depth: jb.depth(),
        jitter_ms: jb.jitter_ms(),
        mode: jb.mode().as_str().to_string(),
    }
}

#[allow(dead_code)]
fn parse_addr(s: &str) -> Option<SocketAddr> {
    s.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::DRIFT_CORRECTION_MAX_RATIO;

    #[test]
    fn recommend_bitrate_high_loss_decreases() {
        // 10% 丢包率应低于基线 128k。
        let r = recommend_bitrate(0.10, 100);
        assert!(r < 128_000, "高丢包应建议更低码率，got {}", r);
        assert!(r >= BITRATE_MIN);
    }

    #[test]
    fn recommend_bitrate_low_loss_increases() {
        // 0.5% 丢包率应高于基线。
        let r = recommend_bitrate(0.005, 100);
        assert!(r > 128_000, "低丢包应建议更高码率，got {}", r);
        assert!(r <= BITRATE_MAX);
    }

    #[test]
    fn recommend_bitrate_mid_loss_baseline() {
        let r = recommend_bitrate(0.03, 100);
        assert_eq!(r, 128_000);
    }

    #[test]
    fn recommend_bitrate_insufficient_samples() {
        assert_eq!(recommend_bitrate(0.10, 10), 0);
    }

    #[test]
    fn status_default_has_phase4_fields() {
        let s = ReceiverStatus::default();
        assert_eq!(s.jitter_ms, 0);
        assert_eq!(s.loss_rate, 0.0);
        assert_eq!(s.recommended_bitrate, 0);
        assert!((s.drift_ratio - 1.0).abs() < 1e-9);
    }

    #[test]
    fn set_jitter_mode_updates_status() {
        let engine = ReceiverEngine::new();
        engine.set_jitter_mode(JitterMode::Stable);
        let s = engine.status();
        assert_eq!(s.jitter_mode, "stable");
    }

    #[test]
    fn drift_resampler_bounded_ratio() {
        // 漂移校正比率应被限制在 ±0.5% 范围内（见 DriftResampler 测试）。
        let mut r = crate::audio::resampler::DriftResampler::new();
        for _ in 0..1000 {
            r.observe(0, 8);
        }
        let ratio = r.ratio();
        assert!(ratio >= 1.0 - DRIFT_CORRECTION_MAX_RATIO - 1e-9);
        assert!(ratio <= 1.0);
    }
}
