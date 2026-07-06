//! 发送端引擎（阶段 5）：采集 → Opus 编码 → 加密 → UDP 发送。
//!
//! 与移动端采集组件协议一致（对齐 `docs/First/11-implementation-spec.md`）。
//! 流程：
//! 1. TCP 连接 Receiver 控制通道。
//! 2. hello / hello_ack → pair_request / pair_response → stream_start / stream_start_ack。
//! 3. 派生 audio_key，启动采集源 + 编码 + UDP 发送循环。
//! 4. 心跳与 stats 周期上报。
//!
//! 同时被 Tauri commands（应用）与 `examples/phase5_loopback.rs`（自测）使用。

use crate::audio::capture::CaptureSource;
use crate::audio::opus_codec::default_codec;
use crate::constants::{
    DEFAULT_STREAM_ID, ENCODE_MS_EWMA_ALPHA, FRAME_SAMPLES_TOTAL,
    PROTOCOL_VERSION, SAMPLE_RATE, SENDER_CONNECT_TIMEOUT_SECS, SENDER_HEARTBEAT_INTERVAL_SECS,
    SENDER_STATS_INTERVAL_SECS,
};
use crate::network::control_server::msg_type;
use crate::network::packet::{encode_packet, AudioPacketHeader};
use crate::pairing::{
    derive_pairing_secret, derive_session_keys, diffie_hellman, sender_proof,
    verify_receiver_proof, EphemeralKeyPair,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use ed25519_dalek::SigningKey;
use parking_lot::Mutex;
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpStream, UdpSocket};
use tokio::task::JoinHandle;
use tokio::time::{interval, timeout};

#[derive(Debug, Clone, Serialize)]
pub struct SenderStatus {
    /// "IDLE" | "CONNECTING" | "PAIRED" | "STREAMING" | "ERROR"
    pub state: String,
    pub target_addr: String,
    pub receiver_device_id: String,
    pub receiver_device_name: String,
    pub packets_sent: u64,
    /// 平均编码耗时（ms，EWMA）。
    pub encode_ms_avg: f32,
    /// 当前发送码率（bps，从 Opus 帧实测）。
    pub bitrate: u32,
    /// 是否走已信任路径（跳过配对码）。
    pub trusted: bool,
    /// 错误信息（state=ERROR 时）。
    pub error: String,
}

impl Default for SenderStatus {
    fn default() -> Self {
        Self {
            state: "IDLE".into(),
            target_addr: String::new(),
            receiver_device_id: String::new(),
            receiver_device_name: String::new(),
            packets_sent: 0,
            encode_ms_avg: 0.0,
            bitrate: 0,
            trusted: false,
            error: String::new(),
        }
    }
}

/// 发送端引擎。
pub struct SenderEngine {
    status: Arc<Mutex<SenderStatus>>,
    running: Arc<AtomicBool>,
    send_task: Mutex<Option<JoinHandle<()>>>,
    control_task: Mutex<Option<JoinHandle<()>>>,
    /// 是否启用音频 RAW Data 转储（来自 main.rs 的 DUMP_ENABLE）。
    dump_enable: bool,
}

impl SenderEngine {
    pub fn new() -> Self {
        Self::with_dump(false)
    }

    /// `dump_enable = true` 时启用采集 PCM / Opus 帧转储。
    pub fn with_dump(dump_enable: bool) -> Self {
        Self {
            status: Arc::new(Mutex::new(SenderStatus::default())),
            running: Arc::new(AtomicBool::new(false)),
            send_task: Mutex::new(None),
            control_task: Mutex::new(None),
            dump_enable,
        }
    }

    /// 启动发送端：握手 + 采集 + 发送。
    ///
    /// - `capture`：采集源（如 WASAPI Loopback / 正弦测试源）。
    /// - `receiver_addr`：Receiver 控制通道地址（ip:port）。
    /// - `pairing_code`：配对码（已信任路径可传空）。
    /// - `sender_device_id` / `sender_device_name`：本机身份。
    /// - `sender_signing`：本机 Ed25519 签名密钥（持久化）。
    /// - `audio_port`：Receiver 音频 UDP 端口。
    #[allow(clippy::too_many_arguments)]
    pub async fn start(
        &self,
        mut capture: Box<dyn CaptureSource>,
        receiver_addr: &str,
        pairing_code: &str,
        sender_device_id: &str,
        sender_device_name: &str,
        sender_signing: &SigningKey,
        audio_port: u16,
    ) -> Result<(), String> {
        if self.running.load(Ordering::SeqCst) {
            return Err("发送端已在运行".into());
        }

        {
            let mut s = self.status.lock();
            *s = SenderStatus::default();
            s.state = "CONNECTING".into();
            s.target_addr = receiver_addr.into();
        }

        self.running.store(true, Ordering::SeqCst);

        // 1) TCP 连接 + 握手。
        let handshake_result = self
            .handshake(
                receiver_addr,
                pairing_code,
                sender_device_id,
                sender_device_name,
                sender_signing,
                audio_port,
            )
            .await;

        let (audio_key, stream_id, tcp_writer, receiver_device_id, receiver_device_name, trusted) =
            match handshake_result {
                Ok(v) => v,
                Err(e) => {
                    self.set_error(&e);
                    self.running.store(false, Ordering::SeqCst);
                    return Err(e);
                }
            };

        {
            let mut s = self.status.lock();
            s.state = "PAIRED".into();
            s.receiver_device_id = receiver_device_id.clone();
            s.receiver_device_name = receiver_device_name.clone();
            s.trusted = trusted;
        }

        // 2) 启动采集源。
        capture.start().map_err(|e| {
            self.set_error(&format!("采集源启动失败：{}", e));
            self.running.store(false, Ordering::SeqCst);
            e
        })?;

        // 3) UDP 发送 socket。
        let udp = UdpSocket::bind("0.0.0.0:0")
            .await
            .map_err(|e| format!("绑定 UDP 发送 socket 失败：{}", e))?;
        // 目标地址 = receiver_addr 的 IP + audio_port。
        let target_ip = receiver_addr
            .split(':')
            .next()
            .unwrap_or("127.0.0.1");
        let udp_target = format!("{}:{}", target_ip, audio_port);
        udp.connect(&udp_target)
            .await
            .map_err(|e| format!("UDP connect 失败：{}", e))?;

        {
            let mut s = self.status.lock();
            s.state = "STREAMING".into();
        }

        // 4) 发送循环任务。
        let status = self.status.clone();
        let running = self.running.clone();
        let dump_enable = self.dump_enable;
        let send_handle = tokio::spawn(async move {
            send_loop(
                capture,
                audio_key,
                stream_id,
                udp,
                status,
                running,
                dump_enable,
            )
            .await;
        });
        *self.send_task.lock() = Some(send_handle);

        // 5) 控制通道任务（心跳 + stats）。
        let status = self.status.clone();
        let running = self.running.clone();
        let control_handle = tokio::spawn(async move {
            control_loop(tcp_writer, stream_id, status, running).await;
        });
        *self.control_task.lock() = Some(control_handle);

        Ok(())
    }

    /// 停止发送端。
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(h) = self.send_task.lock().take() {
            h.abort();
        }
        if let Some(h) = self.control_task.lock().take() {
            h.abort();
        }
        let mut s = self.status.lock();
        s.state = "IDLE".into();
    }

    /// 状态快照。
    pub fn status(&self) -> SenderStatus {
        self.status.lock().clone()
    }

    /// 是否正在运行。
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    fn set_error(&self, msg: &str) {
        let mut s = self.status.lock();
        s.state = "ERROR".into();
        s.error = msg.into();
    }

    /// 控制通道握手：hello → pair_request → pair_response → stream_start → stream_start_ack。
    ///
    /// 返回 (audio_key, stream_id, tcp_writer, receiver_device_id, receiver_device_name, trusted)。
    async fn handshake(
        &self,
        receiver_addr: &str,
        pairing_code: &str,
        sender_device_id: &str,
        sender_device_name: &str,
        sender_signing: &SigningKey,
        audio_port: u16,
    ) -> Result<
        (
            [u8; 32],
            u32,
            tokio::net::tcp::OwnedWriteHalf,
            String,
            String,
            bool,
        ),
        String,
    > {
        let connect = TcpStream::connect(receiver_addr);
        let stream = timeout(
            std::time::Duration::from_secs(SENDER_CONNECT_TIMEOUT_SECS),
            connect,
        )
        .await
        .map_err(|_| format!("连接 {} 超时", receiver_addr))?
        .map_err(|e| format!("连接 {} 失败：{}", receiver_addr, e))?;

        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);

        // hello
        let hello = json!({
            "type": msg_type::HELLO,
            "msg_id": "c-1",
            "ts": now_ms(),
            "protocol_version": PROTOCOL_VERSION,
            "device_id": sender_device_id,
            "device_name": sender_device_name,
            "role": "sender",
            "platform": "desktop",
            "capabilities": { "codec": ["opus"], "sample_rate": SAMPLE_RATE, "channels": 2 },
        });
        send_msg(&mut writer, &hello)
            .await
            .map_err(|e| format!("发送 hello 失败：{}", e))?;
        let hello_ack = recv_msg(&mut reader)
            .await
            .map_err(|e| format!("接收 hello_ack 失败：{}", e))?;
        if hello_ack["type"] != msg_type::HELLO_ACK {
            return Err(format!("期望 hello_ack，got: {}", hello_ack));
        }
        let receiver_device_id = hello_ack["device_id"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let receiver_device_name = hello_ack["device_name"]
            .as_str()
            .unwrap_or("SoundLink Receiver")
            .to_string();
        let trusted = hello_ack["trusted"].as_bool().unwrap_or(false);

        if receiver_device_id.is_empty() {
            return Err("hello_ack 缺少 device_id".into());
        }

        // X25519 密钥对 + 配对秘密。
        let send_kp = EphemeralKeyPair::generate();
        let pairing_secret = if trusted {
            [0u8; 32]
        } else {
            derive_pairing_secret(pairing_code, &receiver_device_id)
        };

        // proof
        let sender_identity_pub_b64 = STANDARD.encode(sender_signing.verifying_key().to_bytes());
        let proof = if trusted {
            String::new()
        } else {
            let proof_bytes = sender_proof(&pairing_secret, &send_kp.public, &receiver_device_id);
            STANDARD.encode(proof_bytes)
        };

        // pair_request
        let pair_req = json!({
            "type": msg_type::PAIR_REQUEST,
            "msg_id": "c-2",
            "ts": now_ms(),
            "device_id": sender_device_id,
            "device_name": sender_device_name,
            "sender_pub": STANDARD.encode(send_kp.public.as_bytes()),
            "sender_identity_pub": sender_identity_pub_b64,
            "proof": proof,
        });
        send_msg(&mut writer, &pair_req)
            .await
            .map_err(|e| format!("发送 pair_request 失败：{}", e))?;
        let pair_resp = recv_msg(&mut reader)
            .await
            .map_err(|e| format!("接收 pair_response 失败：{}", e))?;
        if pair_resp["type"] != msg_type::PAIR_RESPONSE {
            return Err(format!("期望 pair_response，got: {}", pair_resp));
        }
        if pair_resp["result"] != "ok" {
            let code = pair_resp["error"]["code"].as_i64().unwrap_or(0);
            let msg = pair_resp["error"]["message"]
                .as_str()
                .unwrap_or("配对失败");
            return Err(format!("配对失败（code={}）：{}", code, msg));
        }

        let receiver_pub_b64 = pair_resp["receiver_pub"]
            .as_str()
            .ok_or("pair_response 缺少 receiver_pub")?;
        let receiver_pub_bytes = STANDARD
            .decode(receiver_pub_b64)
            .map_err(|e| format!("解码 receiver_pub 失败：{}", e))?;
        if receiver_pub_bytes.len() != 32 {
            return Err("receiver_pub 长度非 32".into());
        }
        let mut rp_arr = [0u8; 32];
        rp_arr.copy_from_slice(&receiver_pub_bytes);
        let receiver_pub = x25519_dalek::PublicKey::from(rp_arr);

        // 校验 receiver_proof（防中间人）。
        if let Some(rp_b64) = pair_resp["proof"].as_str() {
            let rp_bytes = STANDARD
                .decode(rp_b64)
                .map_err(|e| format!("解码 receiver proof 失败：{}", e))?;
            if rp_bytes.len() == 32 {
                let mut rp_arr = [0u8; 32];
                rp_arr.copy_from_slice(&rp_bytes);
                if !verify_receiver_proof(
                    &pairing_secret,
                    &receiver_pub,
                    &send_kp.public,
                    &receiver_device_id,
                    &rp_arr,
                ) {
                    return Err("receiver_proof 校验失败（可能中间人）".into());
                }
            }
        }

        // 派生会话密钥。
        let shared = diffie_hellman(send_kp.secret, &receiver_pub);
        let keys = derive_session_keys(&shared, &pairing_secret);
        let audio_key = keys.audio_key;

        // stream_start
        let stream_id = DEFAULT_STREAM_ID;
        let stream_start = json!({
            "type": msg_type::STREAM_START,
            "msg_id": "c-3",
            "ts": now_ms(),
            "stream_id": stream_id,
            "audio_port": audio_port,
            "codec": "opus",
            "sample_rate": SAMPLE_RATE,
            "channels": 2,
            "frame_duration_ms": 10,
            "bitrate": 128000,
        });
        send_msg(&mut writer, &stream_start)
            .await
            .map_err(|e| format!("发送 stream_start 失败：{}", e))?;
        let ack = recv_msg(&mut reader)
            .await
            .map_err(|e| format!("接收 stream_start_ack 失败：{}", e))?;
        if ack["type"] != msg_type::STREAM_START_ACK || ack["result"] != "ok" {
            return Err(format!("stream_start 被拒绝: {}", ack));
        }

        tracing::info!(
            "握手完成：receiver={} trusted={} audio_port={}",
            receiver_device_id,
            trusted,
            audio_port
        );

        // 关闭 reader，保留 writer 供心跳/stats。
        drop(reader);
        Ok((audio_key, stream_id, writer, receiver_device_id, receiver_device_name, trusted))
    }
}

impl Default for SenderEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for SenderEngine {
    fn drop(&mut self) {
        self.stop();
    }
}

/// 发送循环：每 10ms 采集一帧 → Opus 编码 → 加密 → UDP 发送。
async fn send_loop(
    mut capture: Box<dyn CaptureSource>,
    audio_key: [u8; 32],
    stream_id: u32,
    udp: UdpSocket,
    status: Arc<Mutex<SenderStatus>>,
    running: Arc<AtomicBool>,
    dump_enable: bool,
) {
    let mut codec = default_codec();
    let mut seq: u32 = 0;
    let mut total_samples: u64 = 0;
    let mut encode_ms_ewma: f64 = 0.0;
    let mut bytes_sent: u64 = 0;
    let mut bitrate_start = std::time::Instant::now();
    let mut ticker = interval(std::time::Duration::from_millis(10));

    // 调试：开启时把采集 PCM / Opus 帧写到当前工作目录（覆盖写）。
    let mut pcm_dump: Option<std::fs::File> = None;
    let mut opus_dump: Option<std::fs::File> = None;
    if dump_enable {
        pcm_dump = std::fs::OpenOptions::new()
            .create(true).truncate(true).write(true)
            .open("soundlink_sender_pcm.raw")
            .ok();
        opus_dump = std::fs::OpenOptions::new()
            .create(true).truncate(true).write(true)
            .open("soundlink_sender_opus.bin")
            .ok();
        if pcm_dump.is_some() || opus_dump.is_some() {
            tracing::info!(
                "发送端调试保存已启用：soundlink_sender_pcm.raw / soundlink_sender_opus.bin"
            );
        }
    }
    use std::io::Write as _;

    while running.load(Ordering::SeqCst) {
        ticker.tick().await;

        // 拉取一帧 PCM；数据不足时跳过（不发空包）。
        let pcm = match capture.poll_frame() {
            Some(p) => p,
            None => {
                continue;
            }
        };
        if pcm.len() != FRAME_SAMPLES_TOTAL {
            tracing::warn!(
                "采集帧长度异常：{} != {}",
                pcm.len(),
                FRAME_SAMPLES_TOTAL
            );
            continue;
        }
        total_samples += (FRAME_SAMPLES_TOTAL / 2) as u64;

        // 转储采集后 PCM（i16 LE 交错）。
        if let Some(f) = pcm_dump.as_mut() {
            let mut bytes = Vec::with_capacity(pcm.len() * 2);
            for &s in &pcm {
                bytes.extend_from_slice(&s.to_le_bytes());
            }
            let _ = f.write_all(&bytes);
        }

        // 编码（计时）。
        let enc_start = std::time::Instant::now();
        let frame_bytes = codec.encode(&pcm);
        let enc_elapsed = enc_start.elapsed().as_secs_f64() * 1000.0;
        encode_ms_ewma = ENCODE_MS_EWMA_ALPHA * enc_elapsed + (1.0 - ENCODE_MS_EWMA_ALPHA) * encode_ms_ewma;

        // 转储 Opus 帧（4 字节小端长度前缀 + 数据）。
        if let Some(f) = opus_dump.as_mut() {
            let len = (frame_bytes.len() as u32).to_le_bytes();
            let _ = f.write_all(&len);
            let _ = f.write_all(&frame_bytes);
        }

        // 打包加密 + 发送。
        let mut header = AudioPacketHeader::new(stream_id, seq, total_samples);
        let packet = match encode_packet(&audio_key, &mut header, &frame_bytes) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("打包失败：{:?}", e);
                continue;
            }
        };
        let pkt_len = packet.len() as u64;
        if let Err(e) = udp.send(&packet).await {
            tracing::warn!("UDP 发送失败：{}", e);
        }
        bytes_sent += pkt_len;
        seq = seq.wrapping_add(1);

        // 更新状态。
        {
            let mut s = status.lock();
            s.packets_sent = seq as u64;
            s.encode_ms_avg = encode_ms_ewma as f32;
        }
        // 码率：每秒刷新。
        let elapsed = bitrate_start.elapsed().as_secs_f64();
        if elapsed >= 1.0 {
            let bps = (bytes_sent as f64 * 8.0 / elapsed) as u32;
            bytes_sent = 0;
            bitrate_start = std::time::Instant::now();
            let mut s = status.lock();
            s.bitrate = bps;
        }
    }

    capture.stop();
    tracing::info!("发送循环结束，共发送 {} 帧。", seq);
}

/// 控制通道循环：心跳 + stats 上报。
async fn control_loop(
    mut writer: tokio::net::tcp::OwnedWriteHalf,
    stream_id: u32,
    status: Arc<Mutex<SenderStatus>>,
    running: Arc<AtomicBool>,
) {
    let mut hb_ticker = interval(std::time::Duration::from_secs(SENDER_HEARTBEAT_INTERVAL_SECS));
    let mut stats_ticker = interval(std::time::Duration::from_secs(SENDER_STATS_INTERVAL_SECS));

    while running.load(Ordering::SeqCst) {
        tokio::select! {
            _ = hb_ticker.tick() => {
                let hb = json!({
                    "type": msg_type::HEARTBEAT,
                    "msg_id": "c-hb",
                    "ts": now_ms(),
                });
                if send_msg(&mut writer, &hb).await.is_err() {
                    tracing::warn!("心跳发送失败，控制连接可能已断开。");
                    break;
                }
            }
            _ = stats_ticker.tick() => {
                let s = status.lock().clone();
                let stats = json!({
                    "type": msg_type::STATS,
                    "msg_id": "c-stats",
                    "ts": now_ms(),
                    "stream_id": stream_id,
                    "packets_sent": s.packets_sent,
                    "bitrate": s.bitrate,
                    "encode_ms_avg": s.encode_ms_avg,
                });
                if send_msg(&mut writer, &stats).await.is_err() {
                    tracing::warn!("stats 发送失败。");
                    break;
                }
            }
        }
    }

    // 发送 stream_stop。
    let stop_msg = json!({
        "type": msg_type::STREAM_STOP,
        "msg_id": "c-stop",
        "ts": now_ms(),
        "stream_id": stream_id,
    });
    let _ = send_msg(&mut writer, &stop_msg).await;
    let _ = writer.shutdown().await;
}

async fn send_msg(
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
    msg: &Value,
) -> Result<(), String> {
    let line = format!(
        "{}\n",
        serde_json::to_string(msg).map_err(|e| e.to_string())?
    );
    writer
        .write_all(line.as_bytes())
        .await
        .map_err(|e| e.to_string())
}

async fn recv_msg(
    reader: &mut tokio::io::BufReader<tokio::net::tcp::OwnedReadHalf>,
) -> Result<Value, String> {
    let mut line = String::new();
    let n = reader
        .read_line(&mut line)
        .await
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("控制连接已关闭".into());
    }
    serde_json::from_str(line.trim()).map_err(|e| e.to_string())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_default_idle() {
        let s = SenderStatus::default();
        assert_eq!(s.state, "IDLE");
        assert_eq!(s.packets_sent, 0);
    }

    #[test]
    fn engine_new_not_running() {
        let e = SenderEngine::new();
        assert!(!e.is_running());
        assert_eq!(e.status().state, "IDLE");
    }

    #[test]
    fn stop_sets_idle() {
        let e = SenderEngine::new();
        e.running.store(true, Ordering::SeqCst);
        e.stop();
        assert!(!e.is_running());
        assert_eq!(e.status().state, "IDLE");
    }
}
