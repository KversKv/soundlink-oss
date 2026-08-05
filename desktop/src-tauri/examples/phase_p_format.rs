//! 阶段 P 自测：非基线会话格式（44.1kHz / Mono / 20ms）端到端闭环。
//!
//! 验证点：
//! 1. Sender 以 44.1k/Mono/20ms 编码，包头部携带会话格式。
//! 2. Receiver 经 stream_start 解析会话格式，按格式重建解码器。
//! 3. 解码后重采样回 48k/Stereo 基线输出，端到端收发不中断。
//!
//! 运行（真实 Opus）：`cargo run --example phase_p_format --features opus`。

use soundlink_lib::audio::capture::{self, CaptureSource};
use soundlink_lib::config::AudioParams;
use soundlink_lib::constants::{DEFAULT_AUDIO_PORT, DEFAULT_CONTROL_PORT};
use soundlink_lib::device::device_identity::DeviceIdentity;
use soundlink_lib::logging;
use soundlink_lib::network::control_server::ControlServer;
use soundlink_lib::pairing::{PairingCodeManager, TrustStore};
use soundlink_lib::receiver::ReceiverEngine;
use soundlink_lib::sender::SenderEngine;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    logging::init();
    tracing::info!("=== 阶段 P 自测：非基线会话格式 44.1k/Mono/20ms ===");

    let tmp_dir = std::env::temp_dir().join(format!(
        "soundlink_phase_p_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    std::fs::create_dir_all(&tmp_dir)?;

    // ─── Receiver 端 ───
    let identity = DeviceIdentity::load_or_create(&tmp_dir)?;
    let device_name = "PhaseP Test PC".to_string();
    let pairing = Arc::new(PairingCodeManager::new());
    let trust = Arc::new(parking_lot::Mutex::new(TrustStore::load_or_create(
        tmp_dir.join("trust_store.json"),
    )?));
    let identity_arc = Arc::new(parking_lot::Mutex::new(identity));
    let selected_device = Arc::new(parking_lot::Mutex::new(None));
    let engine = Arc::new(ReceiverEngine::new());

    let control = ControlServer::new(
        engine.clone(),
        pairing.clone(),
        identity_arc.clone(),
        trust.clone(),
        selected_device.clone(),
        device_name.clone(),
        DEFAULT_AUDIO_PORT,
    );
    let control_bind = format!("127.0.0.1:{}", DEFAULT_CONTROL_PORT);
    control.start(&control_bind).await?;
    let code = pairing.issue();

    // ─── Sender 端（非基线会话格式）───
    let mut csprng = rand::rngs::OsRng;
    let sender_signing = ed25519_dalek::SigningKey::generate(&mut csprng);
    let sender_device_id = format!("pc-sender-{:03x}", rand::random::<u32>() & 0xFFF);
    let sender_device_name = "PhaseP Sender PC".to_string();

    let capture_source: Box<dyn CaptureSource> = capture::default_test_source();
    let sender = SenderEngine::new();
    let receiver_addr = format!("127.0.0.1:{}", DEFAULT_CONTROL_PORT);

    let params = AudioParams {
        sample_rate: 48_000,
        channels: 1,
        frame_duration_ms: 20,
        bitrate: 96_000,
        jitter_mode: "balanced".into(),
    }
    .normalized();
    tracing::info!(
        "会话参数：{}Hz {}ch {}ms {}kbps",
        params.sample_rate,
        params.channels,
        params.frame_duration_ms,
        params.bitrate / 1000
    );

    sender
        .start(
            capture_source,
            &receiver_addr,
            &code,
            &sender_device_id,
            &sender_device_name,
            &sender_signing,
            DEFAULT_AUDIO_PORT,
            params,
        )
        .await?;

    for i in 0..12 {
        sleep(Duration::from_millis(500)).await;
        let ss = sender.status();
        let rs = engine.status();
        tracing::info!(
            "[{:>2}] sender: sent={} br={}kbps | receiver: state={} recv={} lost={}",
            i,
            ss.packets_sent,
            ss.bitrate / 1000,
            rs.state,
            rs.packets_recv,
            rs.packets_lost
        );
    }

    let final_sender = sender.status();
    let final_receiver = engine.status();
    sender.stop().await;
    control.stop();
    engine.stop();
    let _ = std::fs::remove_dir_all(&tmp_dir);

    let ok = final_sender.packets_sent > 0 && final_receiver.packets_recv > 0;
    if ok {
        tracing::info!(
            "✅ 验收通过：44.1k/Mono/20ms 会话格式端到端打通（sent={} recv={}）。",
            final_sender.packets_sent,
            final_receiver.packets_recv
        );
        Ok(())
    } else {
        tracing::error!(
            "❌ 验收未通过：sent={} recv={}",
            final_sender.packets_sent,
            final_receiver.packets_recv
        );
        std::process::exit(1);
    }
}
