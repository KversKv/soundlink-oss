//! 阶段 3 自测：配对与设备发现端到端闭环。
//!
//! 同进程模拟 Receiver + Sender，验证：
//! 1. mDNS 广播 `_soundlink._udp.local.`（TXT 见 04）。
//! 2. 控制通道握手：hello / hello_ack。
//! 3. 配对码派生 + X25519 + HMAC 证明 + 会话密钥（§5）。
//! 4. stream_start → UDP 接收 → 状态 = RECEIVING。
//! 5. 信任持久化：配对成功后保存到 trust_store.json。
//! 6. 已信任设备自动重连（跳过配对码）：第二次连接 hello_ack.trusted=true，
//!    pair_request 无 proof，仍能派生会话密钥并启动流。
//!
//! 运行：`cargo run --example control_loopback`。
//! 注：mDNS 在某些环境（CI/无组播）可能不可用；本自测同时验证 mDNS 广播不崩溃，
//! 并通过 127.0.0.1 直接 TCP 连接完成控制面闭环（不依赖 mDNS 解析）。

use soundlink_lib::audio::opus_codec::{default_codec, frame_pcm_len};
use soundlink_lib::constants::{
    DEFAULT_AUDIO_PORT, DEFAULT_CONTROL_PORT, DEFAULT_STREAM_ID, PROTOCOL_VERSION,
    SAMPLES_PER_FRAME_PER_CHANNEL,
};
use soundlink_lib::device::device_identity::DeviceIdentity;
use soundlink_lib::logging;
use soundlink_lib::network::control_server::ControlServer;
use soundlink_lib::network::discovery::MdnsBroadcaster;
use soundlink_lib::network::packet::{encode_packet, AudioPacketHeader};
use soundlink_lib::pairing::{
    derive_pairing_secret, derive_session_keys, diffie_hellman, sender_proof,
    verify_receiver_proof, EphemeralKeyPair, PairingCodeManager, TrustStore,
};
use soundlink_lib::receiver::ReceiverEngine;
use std::f32::consts::TAU;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpStream, UdpSocket};
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    logging::init();
    tracing::info!("=== 阶段 3 自测：配对与设备发现 ===");

    // 准备 Receiver 端组件。
    let tmp_dir = std::env::temp_dir().join(format!(
        "soundlink_phase3_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    std::fs::create_dir_all(&tmp_dir)?;

    let identity = DeviceIdentity::load_or_create(&tmp_dir)?;
    let device_id = identity.device_id.clone();
    let device_name = "Phase3 Test PC".to_string();
    let pairing = Arc::new(PairingCodeManager::new());
    let trust = Arc::new(parking_lot::Mutex::new(TrustStore::load_or_create(
        tmp_dir.join("trust_store.json"),
    )?));
    let identity_arc = Arc::new(parking_lot::Mutex::new(identity));
    let selected_device = Arc::new(parking_lot::Mutex::new(None));
    let engine = Arc::new(ReceiverEngine::new());

    // 1) 启动 mDNS 广播（验证不崩溃；环境无组播时忽略错误）。
    let mdns = MdnsBroadcaster::new();
    match mdns.start(
        &device_id,
        &device_name,
        None,
        DEFAULT_CONTROL_PORT,
        DEFAULT_AUDIO_PORT,
        true,
    ) {
        Ok(()) => tracing::info!("mDNS 广播已启动。"),
        Err(e) => tracing::warn!("mDNS 广播启动失败（环境限制，继续测试）：{}", e),
    }

    // 2) 启动控制服务器。
    let control = ControlServer::new(
        engine.clone(),
        pairing.clone(),
        identity_arc.clone(),
        trust.clone(),
        selected_device.clone(),
        device_name.clone(),
        DEFAULT_AUDIO_PORT,
    );
    let bind = format!("127.0.0.1:{}", DEFAULT_CONTROL_PORT);
    control.start(&bind).await?;
    tracing::info!("控制服务器已启动，监听 {}", bind);

    // 3) 第一次配对（使用配对码）。
    let code = pairing.issue();
    tracing::info!("配对码（自测）：{}", code);
    // Sender 持久化身份（模拟真实设备：首次生成后复用）。
    let mut csprng = rand::rngs::OsRng;
    let sender_signing = ed25519_dalek::SigningKey::generate(&mut csprng);
    let (audio_key_1, sender_identity_pub_b64) = simulate_sender_pair(
        &bind,
        &code,
        &device_id,
        "ios-test-001",
        "Test iPhone",
        false,
        &sender_signing,
    )
    .await?;
    tracing::info!("第一次配对成功，audio_key 已派生。");

    // 发送音频并验证接收。
    send_audio_and_verify(&engine, audio_key_1, DEFAULT_STREAM_ID).await?;
    tracing::info!("第一次流验证通过：packets_recv > 0。");

    // 停止流（stream_stop 等价：直接 stop engine）。
    engine.stop();

    // 验证信任已保存。
    {
        let t = trust.lock();
        assert!(t.is_trusted("ios-test-001"), "信任应已保存");
        assert_eq!(
            t.get("ios-test-001").unwrap().identity_pub_b64,
            sender_identity_pub_b64
        );
        tracing::info!("信任已持久化：device_id=ios-test-001");
    }

    // 4) 第二次配对（已信任，跳过配对码）。复用同一 sender 身份。
    // 重新 issue 配对码（但不应被使用）。
    let _unused_code = pairing.issue();
    tracing::info!("第二次连接（已信任路径，跳过配对码）...");
    let (audio_key_2, _) = simulate_sender_pair(
        &bind,
        "",
        &device_id,
        "ios-test-001",
        "Test iPhone",
        true,
        &sender_signing,
    )
    .await?;
    tracing::info!("已信任路径配对成功，audio_key 已派生。");

    // 验证音频流。
    send_audio_and_verify(&engine, audio_key_2, DEFAULT_STREAM_ID).await?;
    tracing::info!("第二次流验证通过：packets_recv > 0。");
    engine.stop();

    // 5) 清理。
    control.stop();
    mdns.stop();
    engine.stop();

    let _ = std::fs::remove_dir_all(&tmp_dir);

    tracing::info!("=== 阶段 3 自测通过 ===");
    tracing::info!("验收：配对一次后可自动重连（无需配对码）。");
    Ok(())
}

/// 模拟 Sender 端：hello → pair_request → pair_response → stream_start。
///
/// `trusted` = true 时走已信任路径（proof 为空，不传配对码）。
/// `sender_signing` 为 Sender 持久化的 Ed25519 身份（复用以匹配信任存储）。
/// 返回 (audio_key, sender_identity_pub_b64)。
async fn simulate_sender_pair(
    server_addr: &str,
    pairing_code: &str,
    receiver_device_id: &str,
    sender_device_id: &str,
    sender_device_name: &str,
    trusted: bool,
    sender_signing: &ed25519_dalek::SigningKey,
) -> Result<([u8; 32], String), Box<dyn std::error::Error>> {
    let stream = TcpStream::connect(server_addr).await?;
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    // Sender Ed25519 身份（由调用方持久化传入）。
    let sender_identity_pub_b64 = {
        use base64::{engine::general_purpose::STANDARD, Engine};
        STANDARD.encode(sender_signing.verifying_key().to_bytes())
    };

    // hello
    let hello = serde_json::json!({
        "type": "hello",
        "msg_id": "c-1",
        "ts": now_ms(),
        "protocol_version": PROTOCOL_VERSION,
        "device_id": sender_device_id,
        "device_name": sender_device_name,
        "role": "sender",
        "platform": "ios",
        "capabilities": { "codec": ["opus"], "sample_rate": 48000, "channels": 2 },
    });
    send_msg(&mut writer, &hello).await?;
    let hello_ack = recv_msg(&mut reader).await?;
    assert_eq!(hello_ack["type"], "hello_ack");
    assert_eq!(hello_ack["device_id"], receiver_device_id);
    if trusted {
        assert_eq!(hello_ack["trusted"], true, "已信任设备应返回 trusted=true");
    }
    let receiver_identity_pub_b64 = hello_ack["device_id"].as_str().unwrap_or("").to_string(); // 不在此校验

    // X25519 密钥对 + 配对秘密。
    let send_kp = EphemeralKeyPair::generate();
    let pairing_secret = if trusted {
        [0u8; 32]
    } else {
        derive_pairing_secret(pairing_code, receiver_device_id)
    };

    // proof
    let proof: String = if trusted {
        String::new()
    } else {
        let proof_bytes = sender_proof(&pairing_secret, &send_kp.public, receiver_device_id);
        base64_encode(&proof_bytes)
    };

    // pair_request
    let pair_req = serde_json::json!({
        "type": "pair_request",
        "msg_id": "c-2",
        "ts": now_ms(),
        "device_id": sender_device_id,
        "device_name": sender_device_name,
        "sender_pub": base64_encode(send_kp.public.as_bytes()),
        "sender_identity_pub": sender_identity_pub_b64,
        "proof": proof,
    });
    send_msg(&mut writer, &pair_req).await?;
    let pair_resp = recv_msg(&mut reader).await?;
    assert_eq!(pair_resp["type"], "pair_response");
    assert_eq!(
        pair_resp["result"], "ok",
        "pair_response 应为 ok: {}",
        pair_resp
    );

    let receiver_pub_b64 = pair_resp["receiver_pub"]
        .as_str()
        .expect("缺少 receiver_pub");
    let receiver_pub_bytes = base64_decode(receiver_pub_b64);
    assert_eq!(receiver_pub_bytes.len(), 32);
    let mut rp_arr = [0u8; 32];
    rp_arr.copy_from_slice(&receiver_pub_bytes);
    let receiver_pub = x25519_dalek::PublicKey::from(rp_arr);

    // 校验 receiver_proof（已信任路径 pairing_secret=0，proof 仍可校验）。
    if let Some(rp_b64) = pair_resp["proof"].as_str() {
        let rp_bytes = base64_decode(rp_b64);
        let mut rp_arr = [0u8; 32];
        rp_arr.copy_from_slice(&rp_bytes);
        assert!(
            verify_receiver_proof(
                &pairing_secret,
                &receiver_pub,
                &send_kp.public,
                receiver_device_id,
                &rp_arr
            ),
            "receiver_proof 校验失败"
        );
    }

    // 派生会话密钥。
    let shared = diffie_hellman(send_kp.secret, &receiver_pub);
    let keys = derive_session_keys(&shared, &pairing_secret);

    // stream_start
    let stream_start = serde_json::json!({
        "type": "stream_start",
        "msg_id": "c-3",
        "ts": now_ms(),
        "stream_id": DEFAULT_STREAM_ID,
        "audio_port": DEFAULT_AUDIO_PORT,
        "codec": "opus",
        "sample_rate": 48000,
        "channels": 2,
        "frame_duration_ms": 10,
        "bitrate": 128000,
    });
    send_msg(&mut writer, &stream_start).await?;
    let ack = recv_msg(&mut reader).await?;
    assert_eq!(ack["type"], "stream_start_ack");
    assert_eq!(ack["result"], "ok");

    drop(writer);
    drop(reader);
    let _ = receiver_identity_pub_b64;

    Ok((keys.audio_key, sender_identity_pub_b64))
}

/// 发送若干 440Hz 音频包，验证 ReceiverEngine 收到。
async fn send_audio_and_verify(
    engine: &Arc<ReceiverEngine>,
    audio_key: [u8; 32],
    stream_id: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    // 等待接收器就绪。
    sleep(Duration::from_millis(200)).await;

    let sock = UdpSocket::bind("127.0.0.1:0").await?;
    sock.connect(("127.0.0.1", DEFAULT_AUDIO_PORT)).await?;

    let mut codec = default_codec();
    let mut seq: u32 = 0;
    let mut total_samples: u64 = 0;

    for _ in 0..200 {
        let pcm = gen_440hz_frame(total_samples);
        total_samples += SAMPLES_PER_FRAME_PER_CHANNEL as u64;
        let frame_bytes = codec.encode(&pcm);
        let mut header = AudioPacketHeader::new(stream_id, seq, total_samples);
        let packet = encode_packet(&audio_key, &mut header, &frame_bytes)?;
        sock.send(&packet).await?;
        seq = seq.wrapping_add(1);
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    // 等待接收端处理。
    sleep(Duration::from_millis(500)).await;
    let status = engine.status();
    tracing::info!(
        "接收状态: state={} recv={} lost={} dropped={} buffer={}ms",
        status.state,
        status.packets_recv,
        status.packets_lost,
        status.packets_dropped,
        status.buffer_ms
    );
    assert_eq!(status.state, "RECEIVING", "状态应为 RECEIVING");
    assert!(status.packets_recv > 0, "应收到包");
    Ok(())
}

async fn send_msg(
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
    msg: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let line = format!("{}\n", serde_json::to_string(msg)?);
    writer.write_all(line.as_bytes()).await?;
    Ok(())
}

async fn recv_msg(
    reader: &mut tokio::io::BufReader<tokio::net::tcp::OwnedReadHalf>,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let mut line = String::new();
    let n = reader.read_line(&mut line).await?;
    assert!(n > 0, "控制连接已关闭");
    Ok(serde_json::from_str(line.trim())?)
}

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn base64_encode(bytes: &[u8]) -> String {
    use base64::{engine::general_purpose::STANDARD, Engine};
    STANDARD.encode(bytes)
}

fn base64_decode(s: &str) -> Vec<u8> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    STANDARD.decode(s).unwrap_or_default()
}

fn gen_440hz_frame(sample_offset: u64) -> Vec<i16> {
    let freq = 440.0f32;
    let sr = 48_000.0f32;
    let amp = 0.25f32;
    let mut pcm = Vec::with_capacity(frame_pcm_len());
    for i in 0..SAMPLES_PER_FRAME_PER_CHANNEL {
        let t = (sample_offset + i as u64) as f32 / sr;
        let v = (t * freq * TAU).sin() * amp;
        let s = (v * 32767.0).clamp(-32768.0, 32767.0) as i16;
        pcm.push(s);
        pcm.push(s);
    }
    pcm
}
