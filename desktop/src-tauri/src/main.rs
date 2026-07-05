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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// 无 Tauri 外壳时：打印用法提示。Phase 1 验收用 `cargo run --example loopback_sender`。
#[cfg(not(feature = "tauri_app"))]
fn main() {
    soundlink_lib::logging::init();
    println!("SoundLink 桌面核心（无 Tauri 外壳）。");
    println!();
    println!("阶段 1 自测：  cargo run --example loopback_sender");
    println!("真实 Opus：    cargo run --example loopback_sender --features opus");
    println!("GUI 外壳：     cargo build --features tauri_app  (需 MSVC Build Tools + WebView2)");
}
