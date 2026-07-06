// 顶部引用 + 非 tauri 入口
#![cfg_attr(not(feature = "tauri_app"), allow(dead_code))]

#[cfg(feature = "tauri_app")]
fn main() {
    soundlink_lib::logging::init();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(soundlink_lib::commands::AppState::new())
        .invoke_handler(tauri::generate_handler![
            soundlink_lib::commands::start_receiver,
            soundlink_lib::commands::stop_receiver,
            soundlink_lib::commands::get_pairing_code,
            soundlink_lib::commands::list_output_devices,
            soundlink_lib::commands::select_output_device,
            soundlink_lib::commands::get_status,
            soundlink_lib::commands::list_trusted_devices,
            soundlink_lib::commands::remove_trusted_device,
            soundlink_lib::commands::set_device_name,
            soundlink_lib::commands::set_jitter_mode,
            soundlink_lib::commands::get_jitter_mode,
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
    println!("SoundLink 桌面核心（无 Tauri 外壳）。");
    println!();
    println!("阶段 1 自测（音频环回）：  cargo run --example loopback_sender");
    println!("阶段 3 自测（配对发现）：  cargo run --example control_loopback");
    println!("阶段 4 自测（弱网优化）：  cargo run --example phase4_loopback");
    println!("阶段 5 自测（桌面发送端）： cargo run --example phase5_loopback");
    println!("真实 Opus：    cargo run --example loopback_sender --features opus");
    println!("WASAPI 采集：  cargo run --example phase5_loopback --features wasapi  (仅 Windows)");
    println!("GUI 外壳：     cargo build --features tauri_app  (需 MSVC Build Tools + WebView2)");
}
