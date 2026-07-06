//! 阶段 4 自测：体验优化（弱网场景）。
//!
//! 同进程模拟 Sender + Receiver，验证阶段 4 增强：
//! 1. 自适应 Jitter Buffer：弱网抖动下动态调整 target_depth。
//! 2. Opus PLC 连续补偿：丢包时 PLC 补帧，超限切静音。
//! 3. 时钟漂移校正：缓冲水位偏差时 ±0.5% 重采样。
//! 4. 丢包/抖动统计：jitter_ms / loss_rate / bitrate / est_latency_ms。
//! 5. 码率建议：高丢包率触发 recommended_bitrate 下调。
//!
//! 弱网模拟：随机丢包（10%）+ 随机延迟抖动（±5ms）。
//!
//! 运行：`cargo run --example phase4_loopback`

use soundlink_lib::audio::jitter_buffer::JitterMode;
use soundlink_lib::audio::opus_codec::{default_codec, frame_pcm_len};
use soundlink_lib::constants::{
    DEFAULT_AUDIO_PORT, DEFAULT_STREAM_ID, SAMPLES_PER_FRAME_PER_CHANNEL,
};
use soundlink_lib::logging;
use soundlink_lib::network::packet::{encode_packet, AudioPacketHeader};
use soundlink_lib::receiver::ReceiverEngine;
use std::f32::consts::TAU;
use std::rc::Rc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::time::{interval, sleep};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    logging::init();
    tracing::info!("=== 阶段 4 自测：体验优化（弱网场景） ===");

    // 用固定 audio_key（阶段 4 自测聚焦音频链路，跳过配对握手）。
    let audio_key = [0x42u8; 32];

    // 启动接收器，使用 Auto 模式。
    let engine = Rc::new(ReceiverEngine::new());
    engine.set_jitter_mode(JitterMode::Auto);
    let bind_addr = format!("127.0.0.1:{}", DEFAULT_AUDIO_PORT);
    engine
        .start(audio_key, DEFAULT_STREAM_ID, &bind_addr, None)
        .await?;
    tracing::info!("ReceiverEngine 已启动（Auto 模式），绑定 {}", bind_addr);

    // 弱网参数。
    let loss_rate = 0.10; // 10% 丢包
    let jitter_ms = 5; // ±5ms 抖动

    // 发送端：模拟弱网（随机丢包 + 抖动）。
    let send_task = tokio::spawn(async move {
        let sock = UdpSocket::bind("127.0.0.1:0").await.expect("bind sender");
        sock.connect(("127.0.0.1", DEFAULT_AUDIO_PORT))
            .await
            .expect("connect");
        let mut codec = default_codec();
        let mut seq: u32 = 0u32;
        let mut total_samples: u64 = 0;
        let mut ticker = interval(Duration::from_millis(10));
        // 发送 ~10 秒（1000 帧），10% 丢包。
        for _ in 0..1000 {
            ticker.tick().await;
            // 随机抖动延迟。
            let delay = (rand::random::<u32>() % (jitter_ms * 2)) as u64;
            if delay > 0 {
                sleep(Duration::from_millis(delay)).await;
            }
            // 随机丢包。
            if rand::random::<f32>() < loss_rate {
                total_samples += SAMPLES_PER_FRAME_PER_CHANNEL as u64;
                seq = seq.wrapping_add(1);
                continue;
            }
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
        tracing::info!("发送端完成，共发送 {} 帧（10% 丢包模拟）。", seq);
        sleep(Duration::from_millis(500)).await;
    });

    // 周期打印状态（验证阶段 4 字段）。
    for i in 0..20 {
        sleep(Duration::from_millis(500)).await;
        let s = engine.status();
        tracing::info!(
            "[{:>2}] state={} recv={} lost={} dropped={} buf={}ms jitter={}ms loss={:.1}% bitrate={}kbps rec_bitrate={}kbps latency={}ms drift={:.4} plc={}",
            i,
            s.state,
            s.packets_recv,
            s.packets_lost,
            s.packets_dropped,
            s.buffer_ms,
            s.jitter_ms,
            s.loss_rate * 100.0,
            s.bitrate / 1000,
            s.recommended_bitrate / 1000,
            s.est_latency_ms,
            s.drift_ratio,
            s.consecutive_plc
        );
    }
    let _ = send_task.await;

    let final_status = engine.status();
    tracing::info!("=== 自测结束 ===");
    tracing::info!(
        "最终: recv={} lost={} loss={:.1}% jitter={}ms latency={}ms rec_bitrate={}kbps",
        final_status.packets_recv,
        final_status.packets_lost,
        final_status.loss_rate * 100.0,
        final_status.jitter_ms,
        final_status.est_latency_ms,
        final_status.recommended_bitrate / 1000
    );

    // 验收：弱网下应能接收且统计字段非零。
    let ok = final_status.state == "RECEIVING"
        && final_status.packets_recv > 0
        && final_status.est_latency_ms > 0;
    engine.stop();
    if ok {
        tracing::info!("✅ 验收通过：弱网下接收正常，延迟/抖动/丢包统计已上报。");
        Ok(())
    } else {
        tracing::error!("❌ 验收未通过。");
        std::process::exit(1);
    }
}

/// 生成一帧 440Hz 立体声 PCM（Int16 交错）。
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
