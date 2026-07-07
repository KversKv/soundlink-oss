// 顶部引用 + 非 tauri 入口
#![cfg_attr(not(feature = "tauri_app"), allow(dead_code))]
#![cfg_attr(
    all(feature = "tauri_app", not(debug_assertions), windows),
    windows_subsystem = "windows"
)]

/// 调试开关（开发期临时便利）。
///
/// 设为 `true` 后：
/// 1. [`soundlink_lib::pairing::PairingCodeManager`] 生成固定配对码 `12345678`，
///    便于手机端固定码连接（手机端 DEBUG 同步默认填 `12345678`）。
/// 2. 默认开启音频 RAW Data 转储（[DUMP_ENABLE] 跟随此值）。
///
/// 发布前务必改回 `false`。
pub const DEBUG: bool = false;

/// 音频 RAW Data 转储开关。
///
/// `true` 时接收端把 Opus 帧 / 解码后 PCM / 重采样后 PCM 写到当前工作目录：
/// `soundlink_opus.bin` / `soundlink_pcm_decoded.raw` / `soundlink_pcm_resampled.raw`，
/// 发送端把采集 PCM / Opus 帧写到 `soundlink_sender_pcm.raw` / `soundlink_sender_opus.bin`。
///
/// 默认跟随 [DEBUG]；如需在非 DEBUG 模式下独立开启转储，改为显式 `true`。
/// 仍可用环境变量 `SOUNDLINK_DUMP=1` 强制开启（兼容旧用法）。
pub const DUMP_ENABLE: bool = DEBUG;

#[cfg(feature = "tauri_app")]
fn main() {
    soundlink_lib::logging::init();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(soundlink_lib::commands::AppState::new(DEBUG, DUMP_ENABLE))
        .invoke_handler(tauri::generate_handler![
            soundlink_lib::commands::start_receiver,
            soundlink_lib::commands::stop_receiver,
            soundlink_lib::commands::get_pairing_code,
            soundlink_lib::commands::get_desktop_settings,
            soundlink_lib::commands::set_pairing_settings,
            soundlink_lib::commands::list_output_devices,
            soundlink_lib::commands::select_output_device,
            soundlink_lib::commands::get_status,
            soundlink_lib::commands::list_trusted_devices,
            soundlink_lib::commands::remove_trusted_device,
            soundlink_lib::commands::set_device_name,
            soundlink_lib::commands::set_jitter_mode,
            soundlink_lib::commands::get_jitter_mode,
            soundlink_lib::commands::set_volume,
            soundlink_lib::commands::get_volume,
            soundlink_lib::commands::get_audio_params,
            soundlink_lib::commands::set_audio_params,
            soundlink_lib::commands::auto_detect_audio_params,
            soundlink_lib::commands::list_capture_sources,
            soundlink_lib::commands::start_sender,
            soundlink_lib::commands::stop_sender,
            soundlink_lib::commands::get_sender_status,
            soundlink_lib::commands::discover_receivers,
            soundlink_lib::commands::get_role,
            soundlink_lib::commands::set_role,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// 无 Tauri 外壳时：打印用法提示。
#[cfg(not(feature = "tauri_app"))]
fn main() {
    soundlink_lib::logging::init();
    println!(
        "SoundLink 桌面核心（无 Tauri 外壳）。DEBUG={}, DUMP_ENABLE={}",
        DEBUG, DUMP_ENABLE
    );
    println!();
    println!("阶段 1 自测（音频环回）：  cargo run --example loopback_sender");
    println!("阶段 3 自测（配对发现）：  cargo run --example control_loopback");
    println!("阶段 4 自测（弱网优化）：  cargo run --example phase4_loopback");
    println!("阶段 5 自测（桌面发送端）： cargo run --example phase5_loopback");
    println!("真实 Opus：    cargo run --example loopback_sender --features opus");
    println!("WASAPI 采集：  cargo run --example phase5_loopback --features wasapi  (仅 Windows)");
    println!("GUI 外壳：     cargo build --features tauri_app  (需 MSVC Build Tools + WebView2)");
}
