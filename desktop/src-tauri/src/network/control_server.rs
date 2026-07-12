//! 控制通道（TCP）：配对/握手/流控制/心跳。对齐 spec §3 §5 §6。
//!
//! 消息格式：每条 UTF-8 JSON + `\n`。所有消息含 `type`/`msg_id`/`ts`。
//! 状态机（Receiver 视角）：IDLE → HANDSHAKING → PAIRED → RECEIVING → IDLE。

use crate::audio::jitter_buffer::JitterMode;
use crate::config::{AppConfig, AudioParams};
use crate::constants::{
    FRAME_DURATION_MS, HEARTBEAT_TIMEOUT_SECS, OPUS_BITRATE, PROTOCOL_VERSION, SAMPLE_RATE,
};
use crate::device::device_identity::DeviceIdentity;
use crate::pairing::{
    derive_pairing_secret, derive_session_keys, diffie_hellman, receiver_proof,
    verify_sender_proof, EphemeralKeyPair, PairingCodeManager, PairingCodeState, TrustStore,
    TrustedDevice,
};
use crate::receiver::ReceiverEngine;
use base64::{engine::general_purpose::STANDARD, Engine};
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Notify;
use tokio::task::JoinHandle;

/// 消息类型字符串。
pub mod msg_type {
    pub const HELLO: &str = "hello";
    pub const HELLO_ACK: &str = "hello_ack";
    pub const PAIR_REQUEST: &str = "pair_request";
    pub const PAIR_RESPONSE: &str = "pair_response";
    pub const STREAM_START: &str = "stream_start";
    pub const STREAM_START_ACK: &str = "stream_start_ack";
    pub const STREAM_STOP: &str = "stream_stop";
    pub const HEARTBEAT: &str = "heartbeat";
    pub const STATS: &str = "stats";
    pub const CONTROL_ACTION: &str = "control_action";
    pub const CONTROL_ACTION_ACK: &str = "control_action_ack";
    pub const ERROR: &str = "error";
}

/// 错误码（对齐 spec §4）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ErrorCode {
    Ok = 1000,
    Internal = 1001,
    PairingFailed = 1002,
    VersionMismatch = 1003,
    PairingExpired = 1004,
    PairingLocked = 1005,
    NotTrusted = 1006,
    StreamRejected = 1007,
    DecryptFailed = 1008,
    Timeout = 1009,
}

impl ErrorCode {
    pub fn as_i32(self) -> i32 {
        self as i32
    }
}

/// 当前配对会话（pair_response 成功后建立）。
pub struct Session {
    pub sender_device_id: String,
    pub sender_identity_pub_b64: String,
    pub sender_addr: SocketAddr,
    pub audio_key: [u8; 32],
    pub stream_id: u32,
}

/// D4：可选 AppHandle 的类型别名。
/// 仅在 `tauri_app` feature 下为 `tauri::AppHandle`，否则为 `()` 占位以保持 lib 在
/// 非 Tauri 构建下可编译（供 examples 使用）。
#[cfg(feature = "tauri_app")]
pub type AppHandleOpt = Option<tauri::AppHandle>;
#[cfg(not(feature = "tauri_app"))]
pub type AppHandleOpt = Option<()>;

/// 控制服务器共享状态。
pub struct ControlState {
    pub engine: Arc<ReceiverEngine>,
    pub pairing: Arc<PairingCodeManager>,
    pub identity: Arc<Mutex<DeviceIdentity>>,
    pub trust: Arc<Mutex<TrustStore>>,
    pub selected_device: Arc<Mutex<Option<usize>>>,
    pub device_name: String,
    pub audio_port: u16,
    pub config: Option<Arc<Mutex<AppConfig>>>,
    pub config_dir: Option<std::path::PathBuf>,
    pub current_session: Mutex<Option<Session>>,
    pub running: Arc<AtomicBool>,
    pub stop_notify: Arc<Notify>,
    /// D4：可选 AppHandle，用于在配对锁定时 emit `pairing-locked` 事件给前端。
    pub app_handle: AppHandleOpt,
}

/// 控制服务器。
pub struct ControlServer {
    pub state: Arc<ControlState>,
    listener_task: Mutex<Option<JoinHandle<()>>>,
}

impl ControlServer {
    pub fn new(
        engine: Arc<ReceiverEngine>,
        pairing: Arc<PairingCodeManager>,
        identity: Arc<Mutex<DeviceIdentity>>,
        trust: Arc<Mutex<TrustStore>>,
        selected_device: Arc<Mutex<Option<usize>>>,
        device_name: String,
        audio_port: u16,
    ) -> Self {
        Self::with_config(
            engine,
            pairing,
            identity,
            trust,
            selected_device,
            device_name,
            audio_port,
            None,
            None,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_config(
        engine: Arc<ReceiverEngine>,
        pairing: Arc<PairingCodeManager>,
        identity: Arc<Mutex<DeviceIdentity>>,
        trust: Arc<Mutex<TrustStore>>,
        selected_device: Arc<Mutex<Option<usize>>>,
        device_name: String,
        audio_port: u16,
        config: Option<Arc<Mutex<AppConfig>>>,
        config_dir: Option<std::path::PathBuf>,
        app_handle: AppHandleOpt,
    ) -> Self {
        Self {
            state: Arc::new(ControlState {
                engine,
                pairing,
                identity,
                trust,
                selected_device,
                device_name,
                audio_port,
                config,
                config_dir,
                current_session: Mutex::new(None),
                running: Arc::new(AtomicBool::new(false)),
                stop_notify: Arc::new(Notify::new()),
                app_handle,
            }),
            listener_task: Mutex::new(None),
        }
    }

    pub async fn start(&self, bind_addr: &str) -> Result<(), String> {
        if self.state.running.load(Ordering::SeqCst) {
            return Err("控制服务器已在运行".into());
        }
        let listener = TcpListener::bind(bind_addr)
            .await
            .map_err(|e| format!("绑定控制端口 {} 失败：{}", bind_addr, e))?;
        self.state.running.store(true, Ordering::SeqCst);
        tracing::info!("控制服务器监听 {}", bind_addr);

        let state = self.state.clone();
        let handle = tokio::spawn(async move {
            loop {
                if !state.running.load(Ordering::SeqCst) {
                    break;
                }
                match listener.accept().await {
                    Ok((stream, addr)) => {
                        let state = state.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle_connection(stream, addr, state).await {
                                tracing::warn!("控制连接（{}）错误：{}", addr, e);
                            }
                        });
                    }
                    Err(e) => {
                        if state.running.load(Ordering::SeqCst) {
                            tracing::warn!("accept 错误：{}", e);
                        }
                        break;
                    }
                }
            }
        });
        *self.listener_task.lock() = Some(handle);
        Ok(())
    }

    pub fn stop(&self) {
        self.state.running.store(false, Ordering::SeqCst);
        self.state.stop_notify.notify_waiters();
        if let Some(h) = self.listener_task.lock().take() {
            h.abort();
        }
        self.state.engine.stop();
        *self.state.current_session.lock() = None;
    }

    pub fn is_running(&self) -> bool {
        self.state.running.load(Ordering::SeqCst)
    }
}

/// 处理一条控制连接（直至断开）。
async fn handle_connection(
    stream: TcpStream,
    addr: SocketAddr,
    state: Arc<ControlState>,
) -> Result<(), String> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    let mut last_hello_device: Option<String> = None;
    let mut stream_active = false;
    let mut last_seen = Instant::now();
    let timeout_duration = Duration::from_secs(HEARTBEAT_TIMEOUT_SECS);

    loop {
        line.clear();
        let read_result = tokio::select! {
            _ = state.stop_notify.notified() => {
                if stream_active {
                    let stop_msg = json!({
                        "type": msg_type::STREAM_STOP,
                        "msg_id": "s-stop",
                        "ts": now_ms(),
                        "stream_id": state
                            .current_session
                            .lock()
                            .as_ref()
                            .map(|s| s.stream_id)
                            .unwrap_or(0),
                    });
                    let _ = write_msg(&mut writer, &stop_msg).await;
                }
                let _ = writer.shutdown().await;
                break;
            }
            read_result = tokio::time::timeout(timeout_duration, reader.read_line(&mut line)) => read_result,
        };
        let n = match read_result {
            Ok(Ok(n)) => n,
            Ok(Err(e)) => return Err(format!("读控制消息失败：{}", e)),
            Err(_) => {
                if stream_active && last_seen.elapsed() >= timeout_duration {
                    // 心跳超时：iOS 锁屏后主 App 被挂起，TCP 心跳停止，
                    // 但 BroadcastExtension 进程独立运行，UDP 音频仍在发送。
                    // 只断开控制连接，不停止音频接收，让音频流继续。
                    tracing::warn!("控制连接（{}）心跳超时，断开控制连接（音频流保持）。", addr);
                    break;
                }
                continue;
            }
        };
        if n == 0 {
            // 对端关闭连接（EOF）：同上，只断开控制连接，不停止音频流。
            // 音频流由 BroadcastExtension 独立发送，不受主 App 控制连接影响。
            if stream_active {
                tracing::info!("控制连接（{}）对端关闭，音频流保持。", addr);
            }
            break;
        }
        last_seen = Instant::now();
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("JSON 解析失败：{}", e);
                continue;
            }
        };
        let msg_type = msg.get("type").and_then(|v| v.as_str()).unwrap_or("");

        let response: Option<Value> = match msg_type {
            msg_type::HELLO => {
                last_hello_device = msg
                    .get("device_id")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                Some(handle_hello(&msg, &state))
            }
            msg_type::PAIR_REQUEST => Some(handle_pair_request(&msg, &state, addr).await),
            msg_type::STREAM_START => {
                let resp = handle_stream_start(&msg, &state).await;
                if resp.get("type").and_then(|v| v.as_str()) == Some(msg_type::STREAM_START_ACK)
                    && resp.get("result").and_then(|v| v.as_str()) == Some("ok")
                {
                    stream_active = true;
                    last_seen = Instant::now();
                }
                Some(resp)
            }
            msg_type::STREAM_STOP => {
                stream_active = false;
                handle_stream_stop(&msg, &state);
                None
            }
            msg_type::HEARTBEAT => {
                tracing::debug!("heartbeat from {}", addr);
                None
            }
            msg_type::STATS => {
                // 阶段 4：回传 receiver stats（spec §3.8）。
                Some(handle_stats(&msg, &state))
            }
            msg_type::CONTROL_ACTION => Some(handle_control_action(&msg, &state)),
            _ => Some(error_msg(
                &msg,
                ErrorCode::Internal,
                &format!("未知消息类型：{}", msg_type),
            )),
        };

        if let Some(resp) = response {
            write_msg(&mut writer, &resp).await?;
        }
    }

    // 连接断开后清理会话（但不停止已信任关系）。
    let _ = last_hello_device;
    Ok(())
}

async fn write_msg(
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
    msg: &Value,
) -> Result<(), String> {
    let frame = format!(
        "{}\n",
        serde_json::to_string(msg).map_err(|e| e.to_string())?
    );
    writer
        .write_all(frame.as_bytes())
        .await
        .map_err(|e| format!("写控制消息失败：{}", e))
}

/// hello → hello_ack。
fn handle_hello(msg: &Value, state: &ControlState) -> Value {
    let protocol_version = msg
        .get("protocol_version")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u8;
    if protocol_version != PROTOCOL_VERSION {
        return error_msg(msg, ErrorCode::VersionMismatch, "协议版本不兼容");
    }
    let sender_device_id = msg
        .get("device_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let trusted = state.trust.lock().is_trusted(&sender_device_id);
    let identity = state.identity.lock();
    json!({
        "type": msg_type::HELLO_ACK,
        "msg_id": new_msg_id("s"),
        "ts": now_ms(),
        "protocol_version": PROTOCOL_VERSION,
        "device_id": identity.device_id,
        "device_name": state.device_name,
        "pairing_required": true,
        "trusted": trusted,
    })
}

/// pair_request → pair_response。
///
/// - 已信任设备：校验 identity_pub 匹配 → X25519 协商会话密钥（跳过配对码）。
/// - 未信任：校验配对码 proof → X25519 + 保存信任。
async fn handle_pair_request(msg: &Value, state: &ControlState, addr: SocketAddr) -> Value {
    let sender_device_id = msg
        .get("device_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let sender_pub_b64 = msg.get("sender_pub").and_then(|v| v.as_str()).unwrap_or("");
    let sender_identity_pub_b64 = msg
        .get("sender_identity_pub")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let proof_b64 = msg.get("proof").and_then(|v| v.as_str()).unwrap_or("");

    // 解析 sender_pub（X25519, 32B）。
    let sender_pub = match decode_x25519_pub(sender_pub_b64) {
        Some(k) => k,
        None => {
            return pair_error(msg, ErrorCode::PairingFailed, "无效的 sender_pub");
        }
    };

    let identity = state.identity.lock();
    let receiver_device_id = identity.device_id.clone();
    let receiver_identity_pub_b64 = identity.identity_pub_b64();
    drop(identity);

    let recv_kp = EphemeralKeyPair::generate();

    // 是否已信任？
    // P0 安全红线修复（NF-01 A5）：公钥不匹配视为不可信，强制走配对路径，阻断 MITM。
    let trusted_match = {
        let trust = state.trust.lock();
        match trust.get(&sender_device_id) {
            Some(td) => {
                if td.identity_pub_b64 == sender_identity_pub_b64 {
                    true
                } else {
                    tracing::error!(
                        "Sender 身份公钥与已保存的不匹配，强制重新配对（疑似中间人）。saved={} recv={}",
                        td.identity_pub_b64,
                        sender_identity_pub_b64
                    );
                    false
                }
            }
            None => false,
        }
    };

    // 同时产出会话密钥与 receiver 回证用的 pairing_secret。
    let (session_keys, proof_secret) = if trusted_match {
        // 已信任：跳过配对码，直接 X25519（pairing_secret 用全 0 占位）。
        let shared = diffie_hellman(recv_kp.secret, &sender_pub);
        let pairing_secret = [0u8; 32];
        let keys = derive_session_keys(&shared, &pairing_secret);
        (keys, pairing_secret)
    } else {
        // 未信任：校验配对码 proof。
        let pairing_code = match state.pairing.current() {
            Some(c) => c,
            None => {
                return pair_error(msg, ErrorCode::PairingExpired, "无有效配对码");
            }
        };
        let pairing_secret = derive_pairing_secret(&pairing_code, &receiver_device_id);

        let proof_bytes = match decode_32b(proof_b64) {
            Some(b) => b,
            None => {
                return pair_error(msg, ErrorCode::PairingFailed, "无效的 proof");
            }
        };

        if !verify_sender_proof(
            &pairing_secret,
            &sender_pub,
            &receiver_device_id,
            &proof_bytes,
        ) {
            // 校验失败：递增尝试计数。
            match state.pairing.verify("__wrong__") {
                PairingCodeState::Locked => {
                    // D4：通知前端配对已锁定，附带剩余秒数与已用尝试次数。
                    let (is_locked, remaining_secs, attempts) = state.pairing.lock_status();
                    if is_locked {
                        #[cfg(feature = "tauri_app")]
                        if let Some(handle) = &state.app_handle {
                            use tauri::Emitter;
                            let _ = handle.emit(
                                "pairing-locked",
                                json!({
                                    "remaining_secs": remaining_secs,
                                    "remaining_attempts": attempts,
                                }),
                            );
                        }
                        // 非 tauri_app 构建下消除未使用变量告警。
                        #[cfg(not(feature = "tauri_app"))]
                        {
                            let _ = (remaining_secs, attempts);
                        }
                    }
                    return pair_error(msg, ErrorCode::PairingLocked, "尝试次数超限");
                }
                PairingCodeState::Expired => {
                    return pair_error(msg, ErrorCode::PairingExpired, "配对码过期");
                }
                _ => {
                    return pair_error(msg, ErrorCode::PairingFailed, "配对码错误或证明校验失败");
                }
            }
        }

        // proof 校验通过：消费配对码。
        let _ = state.pairing.verify(&pairing_code);

        let shared = diffie_hellman(recv_kp.secret, &sender_pub);
        let keys = derive_session_keys(&shared, &pairing_secret);

        // 保存信任关系。
        let trusted_device = TrustedDevice {
            device_id: sender_device_id.clone(),
            identity_pub_b64: sender_identity_pub_b64.clone(),
            name: msg
                .get("device_name")
                .and_then(|v| v.as_str())
                .map(String::from),
            last_seen: now_secs(),
            host: None,
            control_port: None,
            audio_port: None,
        };
        if let Err(e) = state.trust.lock().add(trusted_device) {
            tracing::warn!("保存信任失败：{}", e);
        }
        (keys, pairing_secret)
    };

    // 计算 receiver 回证（已信任路径也可发送，sender 可选择校验）。
    let rp = receiver_proof(
        &proof_secret,
        &recv_kp.public,
        &sender_pub,
        &receiver_device_id,
    );

    // 保存会话。
    *state.current_session.lock() = Some(Session {
        sender_device_id: sender_device_id.clone(),
        sender_identity_pub_b64: sender_identity_pub_b64.clone(),
        sender_addr: addr,
        audio_key: session_keys.audio_key,
        stream_id: 0,
    });

    json!({
        "type": msg_type::PAIR_RESPONSE,
        "msg_id": new_msg_id("s"),
        "ts": now_ms(),
        "result": "ok",
        "receiver_pub": STANDARD.encode(recv_kp.public.as_bytes()),
        "receiver_identity_pub": receiver_identity_pub_b64,
        "proof": STANDARD.encode(rp),
    })
}

/// stream_start → stream_start_ack（启动 UDP 接收）。
async fn handle_stream_start(msg: &Value, state: &ControlState) -> Value {
    let stream_id = msg.get("stream_id").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
    let audio_port = msg
        .get("audio_port")
        .and_then(|v| v.as_u64())
        .unwrap_or(state.audio_port as u64) as u16;

    let audio_key = {
        let session = state.current_session.lock();
        match session.as_ref() {
            Some(s) => s.audio_key,
            None => {
                return error_msg(msg, ErrorCode::NotTrusted, "未配对，无法启动流");
            }
        }
    };

    // 更新 session 的 stream_id。
    if let Some(s) = state.current_session.lock().as_mut() {
        s.stream_id = stream_id;
    }

    // 若引擎已运行且 audio_key 相同（重连场景），跳过重启避免音频中断。
    // 否则：先停止旧引擎，再用新 key 启动。
    let need_restart = !state.engine.is_running()
        || state.engine.current_audio_key() != audio_key;
    if need_restart {
        if state.engine.is_running() {
            state.engine.stop();
        }

        let bind = format!("0.0.0.0:{}", audio_port);
        let device_index = *state.selected_device.lock();
        if let Err(e) = state
            .engine
            .start(audio_key, stream_id, &bind, device_index)
            .await
        {
            return error_msg(msg, ErrorCode::Internal, &format!("启动接收器失败：{}", e));
        }
    } else {
        // I4：同 key 重连跳过引擎重启，但仍需重置 latency_state，避免码率/漂移统计残留旧值。
        state.engine.reset_latency_state();
        tracing::info!("stream_start：audio_key 未变，跳过引擎重启（重连场景，已重置 latency_state）");
    }

    json!({
        "type": msg_type::STREAM_START_ACK,
        "msg_id": new_msg_id("s"),
        "ts": now_ms(),
        "stream_id": stream_id,
        "result": "ok",
        "receiver_audio_port": audio_port,
    })
}

/// stream_stop：停止 UDP 接收（保留控制连接与信任）。
fn handle_stream_stop(_msg: &Value, state: &ControlState) {
    handle_stream_stop_internal(state);
}

fn handle_stream_stop_internal(state: &ControlState) {
    if state.engine.is_running() {
        state.engine.stop();
    }
    if let Some(s) = state.current_session.lock().as_mut() {
        s.stream_id = 0;
    }
}

/// stats：解析 sender 上报，回传 receiver stats（spec §3.8）。
/// 包含 packets_recv / packets_lost / jitter_ms / buffer_ms / est_latency_ms /
/// loss_rate / bitrate / recommended_bitrate（供 sender 做码率自适应）。
fn handle_stats(msg: &Value, state: &ControlState) -> Value {
    let stream_id = msg.get("stream_id").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let sender_bitrate = msg.get("bitrate").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let sender_packets_sent = msg
        .get("packets_sent")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    tracing::debug!(
        "stats from sender: stream_id={} sent={} bitrate={}",
        stream_id,
        sender_packets_sent,
        sender_bitrate
    );

    let st = state.engine.status();
    json!({
        "type": msg_type::STATS,
        "msg_id": new_msg_id("s"),
        "ts": now_ms(),
        "stream_id": stream_id,
        "packets_recv": st.packets_recv,
        "packets_lost": st.packets_lost,
        "jitter_ms": st.jitter_ms,
        "buffer_ms": st.buffer_ms,
        "est_latency_ms": st.est_latency_ms,
        "loss_rate": st.loss_rate,
        "bitrate": st.bitrate,
        "recommended_bitrate": st.recommended_bitrate,
        "jitter_mode": st.jitter_mode,
    })
}

fn handle_control_action(msg: &Value, state: &ControlState) -> Value {
    let action = msg.get("action").and_then(|v| v.as_str()).unwrap_or("");
    match action {
        "audio.params.update" => handle_audio_params_update(msg, state),
        "media.play_pause"
        | "media.previous"
        | "media.next"
        | "shortcut.set"
        | "shortcut.trigger"
        | "audio.params.probe_request"
        | "audio.params.probe_result" => control_action_ack(msg, action, "accepted", Value::Null),
        _ => control_action_ack(
            msg,
            action,
            "unsupported",
            json!({ "code": ErrorCode::Internal.as_i32(), "message": format!("不支持的控制动作：{}", action) }),
        ),
    }
}

fn handle_audio_params_update(msg: &Value, state: &ControlState) -> Value {
    let Some(payload) = msg.get("payload") else {
        return control_action_ack(
            msg,
            "audio.params.update",
            "rejected",
            json!({ "code": ErrorCode::StreamRejected.as_i32(), "message": "缺少音频参数 payload" }),
        );
    };
    let params = audio_params_from_payload(payload).normalized();
    if let Some(mode) = parse_jitter_mode(&params.jitter_mode) {
        state.engine.set_jitter_mode(mode);
    }
    if let (Some(config), Some(dir)) = (&state.config, &state.config_dir) {
        let mut cfg = config.lock();
        cfg.jitter_mode = params.jitter_mode.clone();
        cfg.audio_params = params.clone();
        if let Err(e) = cfg.save(dir) {
            tracing::warn!("保存音频参数失败：{}", e);
        }
    }
    let restart_required = params.sample_rate != SAMPLE_RATE
        || params.channels != 2
        || params.frame_duration_ms != FRAME_DURATION_MS;
    let message = if restart_required {
        "已保存音频参数并切换 Jitter；采样率/声道/帧长需下次开始流时生效"
    } else {
        "已保存音频参数并切换 Jitter；码率由发送端在后续编码中应用"
    };
    control_action_ack(
        msg,
        "audio.params.update",
        "accepted",
        json!({
            "code": ErrorCode::Ok.as_i32(),
            "message": message,
            "restart_required": restart_required,
            "applied": {
                "jitter_mode": params.jitter_mode,
                "bitrate": params.bitrate
            }
        }),
    )
}

fn audio_params_from_payload(payload: &Value) -> AudioParams {
    let sample_rate = payload
        .get("sample_rate")
        .and_then(|v| v.as_u64())
        .unwrap_or(SAMPLE_RATE as u64) as u32;
    let channels = payload
        .get("channels")
        .and_then(|v| v.as_u64())
        .unwrap_or(2) as u8;
    let frame_duration_ms = payload
        .get("frame_duration_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or(FRAME_DURATION_MS as u64) as u8;
    let bitrate = payload
        .get("bitrate")
        .and_then(|v| v.as_u64())
        .unwrap_or(OPUS_BITRATE as u64) as u32;
    let jitter_mode = payload
        .get("jitter_mode")
        .and_then(|v| v.as_str())
        .map(String::from)
        .or_else(|| jitter_mode_from_ms(payload.get("jitter_ms").and_then(|v| v.as_u64())))
        .unwrap_or_else(|| "balanced".into());
    AudioParams {
        sample_rate,
        channels,
        frame_duration_ms,
        bitrate,
        jitter_mode,
    }
}

fn jitter_mode_from_ms(ms: Option<u64>) -> Option<String> {
    match ms {
        Some(40) => Some("low".into()),
        Some(80) => Some("balanced".into()),
        Some(150) => Some("stable".into()),
        _ => None,
    }
}

fn parse_jitter_mode(mode: &str) -> Option<JitterMode> {
    match mode {
        "low" => Some(JitterMode::Low),
        "balanced" => Some(JitterMode::Balanced),
        "stable" => Some(JitterMode::Stable),
        "auto" => Some(JitterMode::Auto),
        _ => None,
    }
}

fn control_action_ack(msg: &Value, action: &str, result: &str, error: Value) -> Value {
    json!({
        "type": msg_type::CONTROL_ACTION_ACK,
        "msg_id": new_msg_id("s"),
        "ts": now_ms(),
        "reply_to": msg.get("msg_id").and_then(|v| v.as_str()).unwrap_or(""),
        "action": action,
        "result": result,
        "error": error,
    })
}

// --- 工具函数 ---

fn error_msg(req: &Value, code: ErrorCode, message: &str) -> Value {
    json!({
        "type": msg_type::ERROR,
        "msg_id": new_msg_id("s"),
        "ts": now_ms(),
        "reply_to": req.get("msg_id").and_then(|v| v.as_str()).unwrap_or(""),
        "error": { "code": code.as_i32(), "message": message },
    })
}

fn pair_error(_req: &Value, code: ErrorCode, message: &str) -> Value {
    json!({
        "type": msg_type::PAIR_RESPONSE,
        "msg_id": new_msg_id("s"),
        "ts": now_ms(),
        "result": "error",
        "error": { "code": code.as_i32(), "message": message },
    })
}

fn decode_x25519_pub(b64: &str) -> Option<x25519_dalek::PublicKey> {
    let bytes = STANDARD.decode(b64).ok()?;
    if bytes.len() != 32 {
        return None;
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Some(x25519_dalek::PublicKey::from(arr))
}

fn decode_32b(b64: &str) -> Option<[u8; 32]> {
    let bytes = STANDARD.decode(b64).ok()?;
    if bytes.len() != 32 {
        return None;
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Some(arr)
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

fn new_msg_id(prefix: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering as AOrd};
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let n = COUNTER.fetch_add(1, AOrd::SeqCst);
    format!("{}-{}", prefix, n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_code_values() {
        assert_eq!(ErrorCode::PairingFailed.as_i32(), 1002);
        assert_eq!(ErrorCode::Timeout.as_i32(), 1009);
    }

    #[test]
    fn decode_pub_invalid() {
        assert!(decode_x25519_pub("short").is_none());
        assert!(decode_x25519_pub(&STANDARD.encode([0u8; 16])).is_none());
        // 有效 32B base64。
        let valid = STANDARD.encode([0u8; 32]);
        assert!(decode_x25519_pub(&valid).is_some());
    }

    #[test]
    fn pair_error_json_format() {
        let req = json!({"type": "pair_request", "msg_id": "c-1"});
        let resp = pair_error(&req, ErrorCode::PairingFailed, "bad code");
        assert_eq!(resp["type"], "pair_response");
        assert_eq!(resp["result"], "error");
        assert_eq!(resp["error"]["code"], 1002);
    }
}
