//! 接收端引擎：UDP 收包 → AEAD 解密 → Jitter Buffer → Opus 解码(PLC) → cpal 输出。
//!
//! 同时被 Tauri commands（应用）与 `examples/loopback_sender.rs`（自测）使用，
//! 不依赖 Tauri。对齐 spec §9 自测闭环。

use crate::audio::jitter_buffer::{JitterBuffer, JitterFrame, PopResult};
use crate::audio::opus_codec::{default_codec, frame_pcm_len, AudioCodec};
use crate::audio::output::{AudioOutput, PlaybackSource};
use crate::constants::{DEFAULT_AUDIO_PORT, DEFAULT_JITTER_MS, FRAME_DURATION_MS};
use crate::network::packet::decode_packet;
use parking_lot::Mutex;
use serde::Serialize;
use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::task::JoinHandle;

#[derive(Debug, Clone, Serialize)]
pub struct ReceiverStatus {
    pub state: String, // "IDLE" | "RECEIVING" | "ERROR"
    pub packets_recv: u64,
    pub packets_lost: u64,
    pub packets_dropped: u64,
    pub buffer_depth: usize,
    pub buffer_ms: u32,
    pub est_latency_ms: u32,
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
            est_latency_ms: DEFAULT_JITTER_MS,
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
}

impl ReceiverEngine {
    pub fn new() -> Self {
        Self {
            status: Arc::new(Mutex::new(ReceiverStatus::default())),
            jitter: Arc::new(Mutex::new(JitterBuffer::new(DEFAULT_JITTER_MS))),
            codec: Arc::new(Mutex::new(default_codec())),
            running: Arc::new(AtomicBool::new(false)),
            udp_task: Mutex::new(None),
            audio_output: Mutex::new(AudioOutput::new()),
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
            *s = ReceiverStatus::default();
        }
        self.running.store(true, Ordering::SeqCst);

        // cpal 输出。
        let playback = Box::new(PlaybackFromJitter::new(
            self.jitter.clone(),
            self.codec.clone(),
        ));
        if let Err(e) = self.audio_output.lock().start(device_index, playback) {
            tracing::warn!("cpal 输出启动失败（继续收包但不发声）：{}", e);
            // 不致命：仍可验证收包/解密/jitter 链路。
        }

        // UDP 收包任务。
        let status = self.status.clone();
        let jitter = self.jitter.clone();
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
                            let frame = JitterFrame {
                                sequence: dec.header.sequence,
                                timestamp: dec.header.timestamp,
                                data: dec.plaintext,
                            };
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
    }

    /// 状态快照。
    pub fn status(&self) -> ReceiverStatus {
        // 实时刷新丢包/缓冲统计。
        {
            let jb = self.jitter.lock();
            let mut s = self.status.lock();
            s.packets_recv = jb.packets_recv;
            s.packets_lost = jb.packets_lost;
            s.packets_dropped = jb.packets_dropped;
            s.buffer_depth = jb.depth();
            s.buffer_ms = (jb.depth() as u32) * FRAME_DURATION_MS as u32;
        }
        self.status.lock().clone()
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

/// 从 Jitter Buffer + Opus 解码器拉取 PCM 的播放源。
struct PlaybackFromJitter {
    jitter: Arc<Mutex<JitterBuffer>>,
    codec: Arc<Mutex<Box<dyn AudioCodec>>>,
    residue: VecDeque<i16>,
}

impl PlaybackFromJitter {
    fn new(jitter: Arc<Mutex<JitterBuffer>>, codec: Arc<Mutex<Box<dyn AudioCodec>>>) -> Self {
        Self {
            jitter,
            codec,
            residue: VecDeque::with_capacity(frame_pcm_len() * 4),
        }
    }

    fn produce_one_frame(&mut self) -> Vec<i16> {
        let mut jb = self.jitter.lock();
        match jb.pop() {
            PopResult::Frame(f) => {
                drop(jb);
                self.codec.lock().decode(&f.data)
            }
            PopResult::Lost => {
                drop(jb);
                self.codec.lock().decode_plc()
            }
            PopResult::Empty => {
                drop(jb);
                vec![0i16; frame_pcm_len()]
            }
        }
    }
}

impl PlaybackSource for PlaybackFromJitter {
    fn fill(&mut self, out: &mut [i16]) {
        let mut filled = 0;
        while filled < out.len() {
            if self.residue.is_empty() {
                let pcm = self.produce_one_frame();
                self.residue.extend(pcm);
            }
            let need = out.len() - filled;
            let take = need.min(self.residue.len());
            for s in self.residue.drain(..take) {
                out[filled] = s;
                filled += 1;
            }
        }
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
}
fn snapshot(jb: &JitterBuffer) -> JitterStats {
    JitterStats {
        recv: jb.packets_recv,
        lost: jb.packets_lost,
        dropped: jb.packets_dropped,
        depth: jb.depth(),
    }
}

#[allow(dead_code)]
fn parse_addr(s: &str) -> Option<SocketAddr> {
    s.parse().ok()
}
