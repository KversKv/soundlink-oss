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
use crate::audio::opus_codec::codec_with_bitrate;
use crate::config::AudioParams;
use crate::constants::{
    CHANNELS, DEFAULT_STREAM_ID, ENCODE_MS_EWMA_ALPHA, FRAME_DURATION_MS, FRAME_SAMPLES_TOTAL,
    PROTOCOL_VERSION, SAMPLE_RATE, SENDER_CONNECT_TIMEOUT_SECS, SENDER_HEARTBEAT_INTERVAL_SECS,
    SENDER_STATS_INTERVAL_SECS,
};
use crate::network::control_server::msg_type;
use crate::network::packet::{encode_packet, AudioPacketHeader};
use crate::pairing::{
    derive_pairing_secret, derive_session_keys, diffie_hellman, sender_proof,
    verify_receiver_proof, EphemeralKeyPair, TrustStore, TrustedDevice,
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
use tokio::net::{tcp::OwnedReadHalf, tcp::OwnedWriteHalf, TcpStream, UdpSocket};
use tokio::sync::Notify;
use tokio::task::JoinHandle;
use tokio::time::{interval, timeout};

#[derive(Debug, Clone, Serialize)]
pub struct SenderStatus {
    /// "IDLE" | "CONNECTING" | "PAIRED" | "STREAMING" | "DISCONNECTED" | "ERROR"
    pub state: String,
    pub target_addr: String,
    pub receiver_device_id: String,
    pub receiver_device_name: String,
    pub packets_sent: u64,
    /// 平均编码耗时（ms，EWMA）。
    pub encode_ms_avg: f32,
    /// 当前发送码率（bps，从 Opus 帧实测）。
    pub bitrate: u32,
    /// 接收端建议码率（bps，来自 stats）。
    pub recommended_bitrate: u32,
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
            recommended_bitrate: 0,
            trusted: false,
            error: String::new(),
        }
    }
}

/// 发送端引擎。
pub struct SenderEngine {
    status: Arc<Mutex<SenderStatus>>,
    running: Arc<AtomicBool>,
    stop_notify: Arc<Notify>,
    send_task: Mutex<Option<JoinHandle<()>>>,
    control_task: Mutex<Option<JoinHandle<()>>>,
    /// 是否启用音频 RAW Data 转储（来自 main.rs 的 DUMP_ENABLE）。
    dump_enable: bool,
    /// 信任存储：配对成功后保存 Receiver 身份与连接信息。
    trust: Option<Arc<Mutex<TrustStore>>>,
    /// D1：重连相关。allow_reconnect=false 时停止重连（用户主动 stop）。
    allow_reconnect: Arc<AtomicBool>,
    reconnect_task: Mutex<Option<JoinHandle<()>>>,
    /// 状态变化回调（D1）：注入后 control_loop 退出时调用，通知 UI。
    #[allow(clippy::type_complexity)]
    on_state_change: Arc<Mutex<Option<Box<dyn Fn(String, String) + Send + Sync>>>>,
    /// I5：公钥不一致回调（注入后 handshake 检测到 MITM 时调用，通知 UI 弹窗）。
    /// 回调参数：(receiver_device_id, receiver_device_name, saved_pub_b64, recv_pub_b64)。
    #[allow(clippy::type_complexity)]
    on_pubkey_mismatch: Arc<Mutex<Option<Box<dyn Fn(String, String, String, String) + Send + Sync>>>>,
    /// 重连参数（D1）：start 时保存，backoff 重连时复用。
    /// capture_factory 闭包用于每次重连重新构造采集源（WASAPI 不可重用）。
    reconnect_params: Arc<Mutex<Option<ReconnectParams>>>,
}

/// D1：重连所需参数。
#[allow(dead_code)]
struct ReconnectParams {
    receiver_addr: String,
    pairing_code: String,
    sender_device_id: String,
    sender_device_name: String,
    sender_signing: SigningKey,
    audio_port: u16,
    audio_params: AudioParams,
    /// 采集源工厂：每次重连调用以构造新的 CaptureSource。
    capture_factory: Arc<dyn Fn() -> Box<dyn CaptureSource> + Send + Sync>,
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
            stop_notify: Arc::new(Notify::new()),
            send_task: Mutex::new(None),
            control_task: Mutex::new(None),
            dump_enable,
            trust: None,
            allow_reconnect: Arc::new(AtomicBool::new(false)),
            reconnect_task: Mutex::new(None),
            on_state_change: Arc::new(Mutex::new(None)),
            on_pubkey_mismatch: Arc::new(Mutex::new(None)),
            reconnect_params: Arc::new(Mutex::new(None)),
        }
    }

    /// 注入信任存储，启用"记住设备"功能。
    pub fn with_trust(trust: Arc<Mutex<TrustStore>>, dump_enable: bool) -> Self {
        Self {
            status: Arc::new(Mutex::new(SenderStatus::default())),
            running: Arc::new(AtomicBool::new(false)),
            stop_notify: Arc::new(Notify::new()),
            send_task: Mutex::new(None),
            control_task: Mutex::new(None),
            dump_enable,
            trust: Some(trust),
            allow_reconnect: Arc::new(AtomicBool::new(false)),
            reconnect_task: Mutex::new(None),
            on_state_change: Arc::new(Mutex::new(None)),
            on_pubkey_mismatch: Arc::new(Mutex::new(None)),
            reconnect_params: Arc::new(Mutex::new(None)),
        }
    }

    /// D1：注入状态变化回调（commands 层调用，回调内 app.emit）。
    pub fn set_on_state_change(&self, cb: Box<dyn Fn(String, String) + Send + Sync>) {
        *self.on_state_change.lock() = Some(cb);
    }

    /// I5：注入公钥不一致回调（commands 层调用，回调内 app.emit `pubkey-mismatch` 事件）。
    /// 回调参数：(receiver_device_id, receiver_device_name, saved_pub_b64, recv_pub_b64)。
    pub fn set_on_pubkey_mismatch(
        &self,
        cb: Box<dyn Fn(String, String, String, String) + Send + Sync>,
    ) {
        *self.on_pubkey_mismatch.lock() = Some(cb);
    }

    /// 启动发送端：握手 + 采集 + 发送（不启用重连，向后兼容）。
    #[allow(clippy::too_many_arguments)]
    pub async fn start(
        &self,
        capture: Box<dyn CaptureSource>,
        receiver_addr: &str,
        pairing_code: &str,
        sender_device_id: &str,
        sender_device_name: &str,
        sender_signing: &SigningKey,
        audio_port: u16,
        audio_params: AudioParams,
    ) -> Result<(), String> {
        // 不启用重连：allow_reconnect 保持 false。
        self.start_inner(
            capture,
            receiver_addr,
            pairing_code,
            sender_device_id,
            sender_device_name,
            sender_signing,
            audio_port,
            audio_params,
        )
        .await
    }

    /// D1：启动发送端并启用 backoff 重连。`capture_factory` 用于重连时重新构造采集源。
    #[allow(clippy::too_many_arguments)]
    pub async fn start_with_reconnect(
        &self,
        capture_factory: Arc<dyn Fn() -> Box<dyn CaptureSource> + Send + Sync>,
        receiver_addr: &str,
        pairing_code: &str,
        sender_device_id: &str,
        sender_device_name: &str,
        sender_signing: &SigningKey,
        audio_port: u16,
        audio_params: AudioParams,
    ) -> Result<(), String> {
        // 保存重连参数。
        *self.reconnect_params.lock() = Some(ReconnectParams {
            receiver_addr: receiver_addr.into(),
            pairing_code: pairing_code.into(),
            sender_device_id: sender_device_id.into(),
            sender_device_name: sender_device_name.into(),
            sender_signing: sender_signing.clone(),
            audio_port,
            audio_params: audio_params.clone(),
            capture_factory,
        });
        self.allow_reconnect.store(true, Ordering::SeqCst);
        let capture = (self.reconnect_params.lock().as_ref().unwrap().capture_factory)();
        self.start_inner(
            capture,
            receiver_addr,
            pairing_code,
            sender_device_id,
            sender_device_name,
            sender_signing,
            audio_port,
            audio_params,
        )
        .await
    }

    /// 内部启动逻辑：握手 + spawn send_loop + control_loop。
    /// control_loop 退出后若 allow_reconnect=true 则 spawn reconnect_task。
    #[allow(clippy::too_many_arguments)]
    async fn start_inner(
        &self,
        mut capture: Box<dyn CaptureSource>,
        receiver_addr: &str,
        pairing_code: &str,
        sender_device_id: &str,
        sender_device_name: &str,
        sender_signing: &SigningKey,
        audio_port: u16,
        audio_params: AudioParams,
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
                audio_params.clone(),
            )
            .await;

        let (
            audio_key,
            stream_id,
            tcp_reader,
            tcp_writer,
            receiver_device_id,
            receiver_device_name,
            trusted,
            receiver_identity_pub_b64,
        ) = match handshake_result {
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

        // 配对成功后保存信任关系（记住 Receiver）。
        if let Some(trust) = &self.trust {
            let target_ip = receiver_addr.split(':').next().unwrap_or("");
            let control_port = receiver_addr
                .rsplit(':')
                .next()
                .and_then(|p| p.parse::<u16>().ok());
            let trusted_device = TrustedDevice {
                device_id: receiver_device_id.clone(),
                identity_pub_b64: receiver_identity_pub_b64.clone(),
                name: Some(receiver_device_name.clone()),
                last_seen: now_secs(),
                host: Some(target_ip.to_string()),
                control_port,
                audio_port: Some(audio_port),
            };
            if let Err(e) = trust.lock().add(trusted_device) {
                tracing::warn!("保存信任 Receiver 失败：{}", e);
            } else {
                tracing::info!(
                    "已记住 Receiver：{} ({})",
                    receiver_device_id,
                    receiver_device_name
                );
            }
        }

        // 2) 启动采集源。
        if let Err(e) = capture.start() {
            self.set_error(&format!("采集源启动失败：{}", e));
            self.running.store(false, Ordering::SeqCst);
            return Err(e);
        }

        // 3) UDP 发送 socket。
        let udp = match UdpSocket::bind("0.0.0.0:0").await {
            Ok(udp) => udp,
            Err(e) => {
                capture.stop();
                self.set_error(&format!("绑定 UDP 发送 socket 失败：{}", e));
                self.running.store(false, Ordering::SeqCst);
                return Err(format!("绑定 UDP 发送 socket 失败：{}", e));
            }
        };
        // 目标地址 = receiver_addr 的 IP + audio_port。
        let target_ip = receiver_addr.split(':').next().unwrap_or("127.0.0.1");
        let udp_target = format!("{}:{}", target_ip, audio_port);
        if let Err(e) = udp.connect(&udp_target).await {
            capture.stop();
            self.set_error(&format!("UDP connect 失败：{}", e));
            self.running.store(false, Ordering::SeqCst);
            return Err(format!("UDP connect 失败：{}", e));
        }

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
                audio_params.normalized(),
            )
            .await;
        });
        *self.send_task.lock() = Some(send_handle);

        // 5) 控制通道任务（心跳 + stats）。
        let status = self.status.clone();
        let running = self.running.clone();
        let stop_notify = self.stop_notify.clone();
        let on_state_change = self.on_state_change.clone();
        let allow_reconnect = self.allow_reconnect.clone();
        let reconnect_params = self.reconnect_params.clone();
        let stop_notify_rc = self.stop_notify.clone();
        let status_rc = self.status.clone();
        let running_rc = self.running.clone();
        let on_state_change_rc = self.on_state_change.clone();
        let control_handle = tokio::spawn(async move {
            control_loop(
                tcp_reader,
                tcp_writer,
                stream_id,
                status,
                running,
                stop_notify,
                on_state_change,
                allow_reconnect,
                reconnect_params,
                stop_notify_rc,
                status_rc,
                running_rc,
                on_state_change_rc,
            )
            .await;
        });
        *self.control_task.lock() = Some(control_handle);

        Ok(())
    }

    /// 停止发送端。
    pub async fn stop(&self) {
        // D1：先标记不允许重连，避免 stop 触发的 DISCONNECTED 启动 backoff。
        self.allow_reconnect.store(false, Ordering::SeqCst);
        self.running.store(false, Ordering::SeqCst);
        self.stop_notify.notify_waiters();
        let send_handle = self.send_task.lock().take();
        let control_handle = self.control_task.lock().take();
        if let Some(h) = send_handle {
            h.abort();
        }
        if let Some(h) = control_handle {
            if timeout(std::time::Duration::from_secs(1), h).await.is_err() {
                tracing::warn!("等待控制通道停止超时。");
            }
        }
        // D1：清理 reconnect_task（若存在）。
        if let Some(h) = self.reconnect_task.lock().take() {
            h.abort();
        }
        let mut s = self.status.lock();
        if s.state != "DISCONNECTED" && s.state != "ERROR" {
            s.state = "IDLE".into();
        }
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
    /// 返回 (audio_key, stream_id, tcp_reader, tcp_writer,
    ///       receiver_device_id, receiver_device_name, trusted, receiver_identity_pub_b64)。
    // 握手参数均为协议必需字段，聚合成结构体反而增加一层间接，暂保留位置参数。
    #[allow(clippy::too_many_arguments)]
    async fn handshake(
        &self,
        receiver_addr: &str,
        pairing_code: &str,
        sender_device_id: &str,
        sender_device_name: &str,
        sender_signing: &SigningKey,
        audio_port: u16,
        audio_params: AudioParams,
    ) -> Result<
        (
            [u8; 32],
            u32,
            OwnedReadHalf,
            OwnedWriteHalf,
            String,
            String,
            bool,
            String,
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
        let receiver_device_id = hello_ack["device_id"].as_str().unwrap_or("").to_string();
        let receiver_device_name = hello_ack["device_name"]
            .as_str()
            .unwrap_or("SoundLink Receiver")
            .to_string();
        let trusted = hello_ack["trusted"].as_bool().unwrap_or(false);

        if receiver_device_id.is_empty() {
            return Err("hello_ack 缺少 device_id".into());
        }

        // 查询本地信任记录（用于身份一致性校验）。
        let locally_trusted_pub: Option<String> = self.trust.as_ref().and_then(|t| {
            t.lock()
                .get(&receiver_device_id)
                .map(|d| d.identity_pub_b64.clone())
        });

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
            let msg = pair_resp["error"]["message"].as_str().unwrap_or("配对失败");
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

        // 提取 Receiver 身份公钥（Ed25519），用于本地信任校验与保存。
        let receiver_identity_pub_b64 = pair_resp["receiver_identity_pub"]
            .as_str()
            .unwrap_or("")
            .to_string();

        // 身份一致性校验：若本地已信任该 Receiver，核对公钥是否匹配。
        // P0 安全红线修复（NF-01 A5）：公钥不一致直接拒绝，阻断中间人攻击。
        if let Some(saved_pub) = &locally_trusted_pub {
            if !saved_pub.is_empty() && saved_pub.as_str() != receiver_identity_pub_b64.as_str() {
                tracing::error!(
                    "Receiver 身份公钥与已保存的不匹配，拒绝连接（疑似中间人攻击）。saved={} recv={}",
                    saved_pub,
                    receiver_identity_pub_b64
                );
                // I5：通知 UI 弹窗（不阻塞拒绝，仍 return Err）。
                // 安全语义不变：公钥不匹配时仍立即拒绝连接，回调仅作事后告知。
                if let Some(cb) = self.on_pubkey_mismatch.lock().as_ref() {
                    cb(
                        receiver_device_id.clone(),
                        receiver_device_name.clone(),
                        saved_pub.clone(),
                        receiver_identity_pub_b64.clone(),
                    );
                }
                return Err(format!(
                    "Receiver 身份公钥与已保存的不匹配（疑似中间人攻击），请删除该已信任设备后重新配对。saved={} recv={}",
                    saved_pub,
                    receiver_identity_pub_b64
                ));
            }
        }

        // 校验 receiver_proof（防中间人）。
        // P0 安全红线修复（NF-01 A5）：proof 缺失或长度异常视为不可信，要求重新配对。
        let rp_b64 = pair_resp["proof"]
            .as_str()
            .ok_or_else(|| "pair_response 缺少 proof 字段（不可信，请重新配对）".to_string())?;
        let rp_bytes = STANDARD
            .decode(rp_b64)
            .map_err(|e| format!("解码 receiver proof 失败：{}", e))?;
        if rp_bytes.len() != 32 {
            return Err("receiver_proof 长度非 32 字节（不可信，请重新配对）".into());
        }
        let mut rp_arr = [0u8; 32];
        rp_arr.copy_from_slice(&rp_bytes);
        if !verify_receiver_proof(
            &pairing_secret,
            &receiver_pub,
            &send_kp.public,
            &receiver_device_id,
            &rp_arr,
        ) {
            return Err("receiver_proof 校验失败（可能中间人，请重新配对）".into());
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
            "sample_rate": audio_params.sample_rate,
            "channels": audio_params.channels,
            "frame_duration_ms": audio_params.frame_duration_ms,
            "bitrate": audio_params.bitrate,
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

        let reader = reader.into_inner();
        Ok((
            audio_key,
            stream_id,
            reader,
            writer,
            receiver_device_id,
            receiver_device_name,
            trusted,
            receiver_identity_pub_b64,
        ))
    }
}

impl Default for SenderEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for SenderEngine {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        self.stop_notify.notify_waiters();
        if let Some(h) = self.send_task.lock().take() {
            h.abort();
        }
        if let Some(h) = self.control_task.lock().take() {
            h.abort();
        }
    }
}

/// 发送循环：每 10ms 采集一帧 → Opus 编码 → 加密 → UDP 发送。
// 参数为音频热路径所需的独立句柄与配置，避免额外包装带来的解引用开销。
#[allow(clippy::too_many_arguments)]
async fn send_loop(
    mut capture: Box<dyn CaptureSource>,
    audio_key: [u8; 32],
    stream_id: u32,
    udp: UdpSocket,
    status: Arc<Mutex<SenderStatus>>,
    running: Arc<AtomicBool>,
    dump_enable: bool,
    audio_params: AudioParams,
) {
    let mut codec = codec_with_bitrate(audio_params.bitrate);
    tracing::info!(
        "发送端音频参数生效：{}Hz {}ch {}ms {}kbps",
        audio_params.sample_rate,
        audio_params.channels,
        audio_params.frame_duration_ms,
        audio_params.bitrate / 1000
    );
    let mut seq: u32 = 0;
    let mut total_samples: u64 = 0;
    let mut encode_ms_ewma: f64 = 0.0;
    let mut bytes_sent: u64 = 0;
    let mut bitrate_start = std::time::Instant::now();
    let mut ticker = interval(std::time::Duration::from_millis(10));

    // 调试：开启时把采集 PCM / Opus 帧写到当前工作目录（覆盖写）。
    // 注意：release 构建下 dump_enable 始终为 false（由 main.rs DUMP_ENABLE 控制，
    // 且环境变量后门已在 receiver.rs 中通过 cfg!(debug_assertions) 剪除）。
    let mut pcm_dump: Option<std::fs::File> = None;
    let mut opus_dump: Option<std::fs::File> = None;
    if dump_enable {
        pcm_dump = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open("soundlink_sender_pcm.raw")
            .ok();
        opus_dump = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
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
            tracing::warn!("采集帧长度异常：{} != {}", pcm.len(), FRAME_SAMPLES_TOTAL);
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
        encode_ms_ewma =
            ENCODE_MS_EWMA_ALPHA * enc_elapsed + (1.0 - ENCODE_MS_EWMA_ALPHA) * encode_ms_ewma;

        // 转储 Opus 帧（4 字节小端长度前缀 + 数据）。
        if let Some(f) = opus_dump.as_mut() {
            let len = (frame_bytes.len() as u32).to_le_bytes();
            let _ = f.write_all(&len);
            let _ = f.write_all(&frame_bytes);
        }

        // 打包加密 + 发送。
        let mut header = AudioPacketHeader::with_audio_params(
            stream_id,
            seq,
            total_samples,
            SAMPLE_RATE,
            CHANNELS,
            FRAME_DURATION_MS,
        );
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

/// 控制通道循环：心跳 + stats 上报。断开后若 allow_reconnect=true 则 backoff 重连。
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
async fn control_loop(
    reader: OwnedReadHalf,
    mut writer: OwnedWriteHalf,
    stream_id: u32,
    status: Arc<Mutex<SenderStatus>>,
    running: Arc<AtomicBool>,
    stop_notify: Arc<Notify>,
    on_state_change: Arc<Mutex<Option<Box<dyn Fn(String, String) + Send + Sync>>>>,
    allow_reconnect: Arc<AtomicBool>,
    _reconnect_params: Arc<Mutex<Option<ReconnectParams>>>,
    // 用于重连后再次 spawn control_loop 的克隆（自引用循环）。
    _stop_notify_rc: Arc<Notify>,
    _status_rc: Arc<Mutex<SenderStatus>>,
    _running_rc: Arc<AtomicBool>,
    _on_state_change_rc: Arc<Mutex<Option<Box<dyn Fn(String, String) + Send + Sync>>>>,
) {
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    let mut hb_ticker = interval(std::time::Duration::from_secs(
        SENDER_HEARTBEAT_INTERVAL_SECS,
    ));
    let mut stats_ticker = interval(std::time::Duration::from_secs(SENDER_STATS_INTERVAL_SECS));

    while running.load(Ordering::SeqCst) {
        tokio::select! {
            _ = stop_notify.notified() => {
                break;
            }
            read = reader.read_line(&mut line) => {
                match read {
                    Ok(0) => {
                        tracing::warn!("控制连接已由接收端关闭。");
                        mark_disconnected(&status, &running, &on_state_change, "接收端已断开", false);
                        break;
                    }
                    Ok(_) => {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() {
                            handle_control_message(trimmed, &status, &running, &on_state_change);
                        }
                        line.clear();
                    }
                    Err(e) => {
                        tracing::warn!("读取控制消息失败：{}", e);
                        mark_disconnected(&status, &running, &on_state_change, &format!("控制连接读取失败：{}", e), false);
                        break;
                    }
                }
            }
            _ = hb_ticker.tick() => {
                let hb = json!({
                    "type": msg_type::HEARTBEAT,
                    "msg_id": "c-hb",
                    "ts": now_ms(),
                });
                if send_msg(&mut writer, &hb).await.is_err() {
                    tracing::warn!("心跳发送失败，控制连接可能已断开。");
                    mark_disconnected(&status, &running, &on_state_change, "心跳发送失败", false);
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
                    mark_disconnected(&status, &running, &on_state_change, "stats 发送失败", false);
                    break;
                }
            }
        }
    }

    let stop_msg = json!({
        "type": msg_type::STREAM_STOP,
        "msg_id": "c-stop",
        "ts": now_ms(),
        "stream_id": stream_id,
    });
    let _ = send_msg(&mut writer, &stop_msg).await;
    let _ = writer.shutdown().await;

    // D1：backoff 重连。5s / 10s / 30s 三档，成功则 spawn 新任务（此处仅标记 RECONNECTING + 通知 UI）。
    // 实际重连由 commands 层在收到 sender-state-changed 事件后调用 start_sender 命令。
    if !allow_reconnect.load(Ordering::SeqCst) {
        return;
    }
    let backoffs = [5u64, 10, 30];
    for (i, &delay) in backoffs.iter().enumerate() {
        if !allow_reconnect.load(Ordering::SeqCst) {
            return;
        }
        {
            let mut s = status.lock();
            s.state = "RECONNECTING".into();
            s.error = format!("{}s 后重连（第 {} 次）", delay, i + 1);
        }
        if let Some(cb) = on_state_change.lock().as_ref() {
            cb("RECONNECTING".into(), format!("{}s 后重连（第 {} 次）", delay, i + 1));
        }
        tracing::info!("D1：{}s 后开始第 {} 次重连", delay, i + 1);
        tokio::select! {
            _ = stop_notify.notified() => { return; }
            _ = tokio::time::sleep(std::time::Duration::from_secs(delay)) => {}
        }
        if !allow_reconnect.load(Ordering::SeqCst) {
            return;
        }
        // 通知 UI 触发重连（UI 调用 start_sender 命令）。
        if let Some(cb) = on_state_change.lock().as_ref() {
            cb("RECONNECT_NOW".into(), format!("第 {} 次重连", i + 1));
        }
        // 等待 UI 重连或超时。若 30s 内未恢复 running，继续下一档 backoff。
        let wait_start = std::time::Instant::now();
        loop {
            if running.load(Ordering::SeqCst) {
                // UI 已成功重连。
                return;
            }
            if !allow_reconnect.load(Ordering::SeqCst) {
                return;
            }
            if wait_start.elapsed() > std::time::Duration::from_secs(30) {
                break;
            }
            tokio::select! {
                _ = stop_notify.notified() => { return; }
                _ = tokio::time::sleep(std::time::Duration::from_millis(500)) => {}
            }
        }
    }
    // 三次失败转手动。
    {
        let mut s = status.lock();
        s.state = "DISCONNECTED".into();
        s.error = "重连失败，请手动重试".into();
    }
    if let Some(cb) = on_state_change.lock().as_ref() {
        cb("DISCONNECTED".into(), "重连失败，请手动重试".into());
    }
}

/// D1：统一标记断开并通知回调。
#[allow(clippy::type_complexity)]
fn mark_disconnected(
    status: &Arc<Mutex<SenderStatus>>,
    running: &Arc<AtomicBool>,
    on_state_change: &Arc<Mutex<Option<Box<dyn Fn(String, String) + Send + Sync>>>>,
    reason: &str,
    is_error: bool,
) {
    running.store(false, Ordering::SeqCst);
    let new_state = if is_error { "ERROR" } else { "DISCONNECTED" };
    let (state, error) = {
        let mut s = status.lock();
        s.state = new_state.into();
        s.error = reason.into();
        (s.state.clone(), s.error.clone())
    };
    if let Some(cb) = on_state_change.lock().as_ref() {
        cb(state, error);
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn handle_control_message(
    line: &str,
    status: &Arc<Mutex<SenderStatus>>,
    running: &Arc<AtomicBool>,
    on_state_change: &Arc<Mutex<Option<Box<dyn Fn(String, String) + Send + Sync>>>>,
) {
    let Ok(msg) = serde_json::from_str::<Value>(line) else {
        return;
    };
    match msg.get("type").and_then(|v| v.as_str()).unwrap_or("") {
        msg_type::ERROR => {
            let message = msg
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|v| v.as_str())
                .unwrap_or("接收端返回错误");
            mark_disconnected(status, running, on_state_change, message, true);
        }
        msg_type::STREAM_STOP => {
            mark_disconnected(status, running, on_state_change, "接收端已停止接收", false);
        }
        msg_type::CONTROL_ACTION => {
            tracing::debug!(
                "收到控制动作：{}",
                msg.get("action").and_then(|v| v.as_str()).unwrap_or("")
            );
        }
        msg_type::CONTROL_ACTION_ACK => {
            tracing::debug!(
                "收到控制动作回执：{}",
                msg.get("action").and_then(|v| v.as_str()).unwrap_or("")
            );
        }
        msg_type::STATS => {
            if let Some(recommended) = msg.get("recommended_bitrate").and_then(|v| v.as_u64()) {
                let mut s = status.lock();
                s.recommended_bitrate = recommended as u32;
            }
        }
        _ => {}
    }
}

async fn send_msg(writer: &mut OwnedWriteHalf, msg: &Value) -> Result<(), String> {
    let line = format!(
        "{}\n",
        serde_json::to_string(msg).map_err(|e| e.to_string())?
    );
    writer
        .write_all(line.as_bytes())
        .await
        .map_err(|e| e.to_string())
}

async fn recv_msg(reader: &mut BufReader<OwnedReadHalf>) -> Result<Value, String> {
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

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
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

    #[tokio::test]
    async fn stop_sets_idle() {
        let e = SenderEngine::new();
        e.running.store(true, Ordering::SeqCst);
        e.stop().await;
        assert!(!e.is_running());
        assert_eq!(e.status().state, "IDLE");
    }
}
