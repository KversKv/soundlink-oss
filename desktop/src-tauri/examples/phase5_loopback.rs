//! 阶段 5 自测：桌面发送端 → 桌面接收端端到端闭环。
//!
//! 同进程模拟 Receiver + Sender（双电脑互传的最小验证）：
//! 1. 启动 ReceiverEngine + 控制服务器 + mDNS 广播。
//! 2. 启动 SenderEngine（SineWaveCapture 采集源）连接本地 Receiver。
//! 3. 完整握手（hello / pair_request / pair_response / stream_start）。
//! 4. Sender 采集 440Hz → Opus 编码 → 加密 → UDP 发送；Receiver 接收并播放。
//! 5. 验证 sender.packets_sent > 0 且 receiver.packets_recv > 0。
//!
//! 运行：`cargo run --example phase5_loopback`（默认 passthrough）。
//! 真实 Opus：`cargo run --example phase5_loopback --features opus`。
//! WASAPI loopback 采集：`cargo run --example phase5_loopback --features wasapi`（仅 Windows）。

use soundlink_lib::audio::capture::{self, CaptureSource};
use soundlink_lib::constants::{DEFAULT_AUDIO_PORT, DEFAULT_CONTROL_PORT};
use soundlink_lib::device::device_identity::DeviceIdentity;
use soundlink_lib::logging;
use soundlink_lib::network::control_server::ControlServer;
use soundlink_lib::network::discovery::MdnsBroadcaster;
use soundlink_lib::pairing::{PairingCodeManager, TrustStore};
use soundlink_lib::receiver::ReceiverEngine;
use soundlink_lib::sender::SenderEngine;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    logging::init();
    tracing::info!("=== 阶段 5 自测：桌面发送端（双电脑互传）===");

    // 准备临时目录（信任存储）。
    let tmp_dir = std::env::temp_dir().join(format!(
        "soundlink_phase5_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    std::fs::create_dir_all(&tmp_dir)?;

    // ─── Receiver 端 ───
    let identity = DeviceIdentity::load_or_create(&tmp_dir)?;
    let receiver_device_id = identity.device_id.clone();
    let device_name = "Phase5 Test PC".to_string();
    let pairing = Arc::new(PairingCodeManager::new());
    let trust = Arc::new(parking_lot::Mutex::new(TrustStore::load_or_create(
        tmp_dir.join("trust_store.json"),
    )?));
    let identity_arc = Arc::new(parking_lot::Mutex::new(identity));
    let selected_device = Arc::new(parking_lot::Mutex::new(None));
    let engine = Arc::new(ReceiverEngine::new());

    // mDNS 广播（验证不崩溃）。
    let mdns = MdnsBroadcaster::new();
    match mdns.start(
        &receiver_device_id,
        &device_name,
        None,
        DEFAULT_CONTROL_PORT,
        DEFAULT_AUDIO_PORT,
        true,
    ) {
        Ok(()) => tracing::info!("mDNS 广播已启动。"),
        Err(e) => tracing::warn!("mDNS 广播启动失败（环境限制，继续）：{}", e),
    }

    // 控制服务器。
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
    tracing::info!("Receiver 控制服务器监听 {}", control_bind);

    let code = pairing.issue();
    tracing::info!("配对码（自测）：{}", code);

    // ─── Sender 端 ───
    // Sender 持久化身份（模拟真实设备）。
    let mut csprng = rand::rngs::OsRng;
    let sender_signing = ed25519_dalek::SigningKey::generate(&mut csprng);
    let sender_device_id = format!("pc-sender-{:03x}", rand::random::<u32>() & 0xFFF);
    let sender_device_name = "Phase5 Sender PC".to_string();

    // 选择采集源：
    // - wasapi feature + Windows：WASAPI Loopback（真实系统音频）
    // - 否则：440Hz 正弦波（自测）
    let capture_source: Box<dyn CaptureSource> = make_capture_source();
    tracing::info!("采集源：{}", capture_source.name());

    let sender = SenderEngine::new();
    let receiver_addr = format!("127.0.0.1:{}", DEFAULT_CONTROL_PORT);
    tracing::info!("Sender 连接 {}", receiver_addr);

    sender
        .start(
            capture_source,
            &receiver_addr,
            &code,
            &sender_device_id,
            &sender_device_name,
            &sender_signing,
            DEFAULT_AUDIO_PORT,
        )
        .await?;
    tracing::info!("Sender 已启动，进入 STREAMING 状态。");

    // ─── 监控运行 ~6 秒 ───
    for i in 0..12 {
        sleep(Duration::from_millis(500)).await;
        let ss = sender.status();
        let rs = engine.status();
        tracing::info!(
            "[{:>2}] sender: state={} sent={} enc={:.1}ms br={}kbps | receiver: state={} recv={} lost={} buf={}ms",
            i,
            ss.state,
            ss.packets_sent,
            ss.encode_ms_avg,
            ss.bitrate / 1000,
            rs.state,
            rs.packets_recv,
            rs.packets_lost,
            rs.buffer_ms
        );
    }

    // ─── 验收 ───
    let final_sender = sender.status();
    let final_receiver = engine.status();

    sender.stop();
    control.stop();
    mdns.stop();
    engine.stop();
    let _ = std::fs::remove_dir_all(&tmp_dir);

    tracing::info!("=== 自测结束 ===");
    tracing::info!("Sender 最终：state={} packets_sent={}", final_sender.state, final_sender.packets_sent);
    tracing::info!("Receiver 最终：state={} packets_recv={} packets_lost={}", final_receiver.state, final_receiver.packets_recv, final_receiver.packets_lost);

    let ok = final_sender.state == "STREAMING"
        && final_sender.packets_sent > 0
        && final_receiver.state == "RECEIVING"
        && final_receiver.packets_recv > 0;

    if ok {
        tracing::info!("✅ 验收通过：Sender 已发送包，Receiver 已接收包（双电脑互传链路打通）。");
        Ok(())
    } else {
        tracing::error!("❌ 验收未通过。");
        std::process::exit(1);
    }
}

/// 构造采集源：优先 WASAPI loopback（Windows + wasapi feature），否则正弦波。
fn make_capture_source() -> Box<dyn CaptureSource> {
    #[cfg(all(windows, feature = "wasapi"))]
    {
        Box::new(capture::wasapi_loopback::WasapiLoopbackCapture::new())
    }
    #[cfg(not(all(windows, feature = "wasapi")))]
    {
        capture::default_test_source()
    }
}
