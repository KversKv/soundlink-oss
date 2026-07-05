//! 环回自测（spec §9）。
//!
//! 同进程模拟 Sender + Receiver：
//! 1. 生成配对码，本地完成 §5 握手（X25519 + HMAC），派生 audio_key。
//! 2. 启动 ReceiverEngine（绑定 127.0.0.1:47811，默认输出设备）。
//! 3. 生成 440Hz 正弦 → 编码 → AudioPacket 加密 → UDP 发到 127.0.0.1:47811。
//! 4. 周期打印 get_status()，验证 state=RECEIVING、packets_lost≈0。
//!
//! 运行：`cargo run --example loopback_sender`（默认 passthrough）。
//! 真实 Opus：`cargo run --example loopback_sender --features opus`。

use soundlink_lib::audio::opus_codec::{default_codec, frame_pcm_len};
use soundlink_lib::constants::{
    DEFAULT_AUDIO_PORT, DEFAULT_STREAM_ID, SAMPLES_PER_FRAME_PER_CHANNEL,
};
use soundlink_lib::logging;
use soundlink_lib::network::packet::{encode_packet, AudioPacketHeader};
use soundlink_lib::pairing::{
    derive_pairing_secret, derive_session_keys, diffie_hellman, verify_receiver_proof,
    EphemeralKeyPair, PairingCodeManager,
};
use soundlink_lib::receiver::ReceiverEngine;
use std::f32::consts::TAU;
use std::rc::Rc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::time::{interval, sleep};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    logging::init();
    tracing::info!("=== SoundLink 环回自测 ===");

    // 1) 配对码 + 同进程握手。
    let pairing_mgr = PairingCodeManager::new();
    let code = pairing_mgr.issue();
    tracing::info!("配对码（自测，不对外）：{}", code);
    let receiver_device_id = "pc-loopback-self".to_string();
    let pairing_secret = derive_pairing_secret(&code, &receiver_device_id);

    let recv_kp = EphemeralKeyPair::generate();
    let send_kp = EphemeralKeyPair::generate();
    let shared = diffie_hellman(recv_kp.secret, &send_kp.public);
    let keys = derive_session_keys(&shared, &pairing_secret);
    let audio_key = keys.audio_key;

    // （可选）校验 receiver_proof 证明握手完整。
    let rp = soundlink_lib::pairing::receiver_proof(
        &pairing_secret,
        &recv_kp.public,
        &send_kp.public,
        &receiver_device_id,
    );
    assert!(verify_receiver_proof(
        &pairing_secret,
        &recv_kp.public,
        &send_kp.public,
        &receiver_device_id,
        &rp
    ));
    tracing::info!("握手完成，audio_key 已派生。");

    // 2) 启动接收器。
    let engine = Rc::new(ReceiverEngine::new());
    let bind_addr = format!("127.0.0.1:{}", DEFAULT_AUDIO_PORT);
    engine
        .start(audio_key, DEFAULT_STREAM_ID, &bind_addr, None)
        .await?;
    tracing::info!("ReceiverEngine 已启动，绑定 {}", bind_addr);

    // 3) 发送 440Hz（独立任务；不持有 ReceiverEngine——cpal::Stream 非 Send，
    //    跨线程传递 Arc<ReceiverEngine> 会使 tokio::spawn 的 future 非 Send）。
    let send_task = tokio::spawn(async move {
        let sock = UdpSocket::bind("127.0.0.1:0").await.expect("bind sender");
        sock.connect(("127.0.0.1", DEFAULT_AUDIO_PORT))
            .await
            .expect("connect");
        let mut codec = default_codec();
        let mut seq: u32 = 0u32;
        let mut total_samples: u64 = 0;
        let mut ticker = interval(Duration::from_millis(10));
        // 发送 ~6 秒（600 帧）。
        for _ in 0..600 {
            ticker.tick().await;
            let pcm = gen_440hz_frame(total_samples);
            total_samples += SAMPLES_PER_FRAME_PER_CHANNEL as u64;
            let frame_bytes = codec.encode(&pcm);
            let mut header = AudioPacketHeader::new(DEFAULT_STREAM_ID, seq, total_samples);
            let packet = match encode_packet(&audio_key, &mut header, &frame_bytes) {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!("打包失败：{:?}", e);
                    continue;
                }
            };
            if let Err(e) = sock.send(&packet).await {
                tracing::warn!("发送失败：{}", e);
            }
            seq = seq.wrapping_add(1);
        }
        tracing::info!("发送端完成，共发送 {} 帧。", seq);
        // 留一点时间让接收端把尾包播放完。
        sleep(Duration::from_millis(300)).await;
    });

    // 4) 周期打印状态（主任务内轮询；engine 非 Send，不能跨 tokio::spawn）。
    for _ in 0..12 {
        sleep(Duration::from_millis(500)).await;
        let s = engine.status();
        tracing::info!(
            "状态: state={} recv={} lost={} dropped={} buffer={}ms",
            s.state,
            s.packets_recv,
            s.packets_lost,
            s.packets_dropped,
            s.buffer_ms
        );
    }
    let _ = send_task.await;

    let final_status = engine.status();
    tracing::info!("=== 自测结束 ===");
    tracing::info!("最终状态: {:?}", final_status);

    let ok = final_status.state == "RECEIVING"
        && final_status.packets_recv > 0
        && final_status.packets_lost == 0;
    engine.stop();
    if ok {
        tracing::info!("✅ 验收通过：state=RECEIVING，packets_lost≈0。");
        Ok(())
    } else {
        tracing::error!("❌ 验收未通过。");
        std::process::exit(1);
    }
}

/// 生成一帧 440Hz 立体声 PCM（Int16 交错）。`sample_offset` 为本帧首个样本的全局序号。
fn gen_440hz_frame(sample_offset: u64) -> Vec<i16> {
    let freq = 440.0f32;
    let sr = 48_000.0f32;
    let amp = 0.25f32; // 避免削顶
    let mut pcm = Vec::with_capacity(frame_pcm_len());
    for i in 0..SAMPLES_PER_FRAME_PER_CHANNEL {
        let t = (sample_offset + i as u64) as f32 / sr;
        let v = (t * freq * TAU).sin() * amp;
        let s = (v * 32767.0).clamp(-32768.0, 32767.0) as i16;
        pcm.push(s); // L
        pcm.push(s); // R
    }
    pcm
}
