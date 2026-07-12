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
/// 环境变量 `SOUNDLINK_DUMP=1` 仅在 debug 构建生效（release 构建通过 `cfg!(debug_assertions)`
/// 完全剪除，避免环境变量后门绕过）。
pub const DUMP_ENABLE: bool = DEBUG;

#[cfg(feature = "tauri_app")]
fn main() {
    use tauri::Manager;
    soundlink_lib::logging::init();
    install_panic_hook();
    tauri::Builder::default()
        // 单实例锁定：必须最早注册。二次启动聚焦既有窗口（D2）。
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        // 窗口大小/位置记忆（E2）。
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--autostarted"]),
        ))
        // I2：全局快捷键。Ctrl+Shift+P 切换角色、Ctrl+Shift+S 显示主窗口。
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        use tauri::Emitter;
                        let kind = match shortcut.to_string().as_str() {
                            "Ctrl+Shift+P" => "toggle-role",
                            "Ctrl+Shift+S" => "show-window",
                            _ => return,
                        };
                        let _ = app.emit("global-shortcut", serde_json::json!({ "kind": kind }));
                    }
                })
                .build(),
        )
        .manage(soundlink_lib::commands::AppState::new(DEBUG, DUMP_ENABLE))
        .setup(|app| {
            if let Err(e) = soundlink_lib::commands::tray::setup_tray(app) {
                tracing::warn!("托盘初始化失败：{}（应用继续启动）", e);
            }
            // I2：注册全局快捷键。
            use tauri_plugin_global_shortcut::GlobalShortcutExt;
            let shortcuts = ["Ctrl+Shift+P", "Ctrl+Shift+S"];
            for sc in shortcuts {
                if let Err(e) = app.global_shortcut().register(sc) {
                    tracing::warn!("注册全局快捷键 {} 失败：{}", sc, e);
                }
            }
            // D5：identity 加载失败时通知前端提示用户重新配对。
            let state: tauri::State<soundlink_lib::commands::AppState> = app.state();
            if state.identity_load_failed {
                use tauri::Emitter;
                let _ = app.emit(
                    "identity-load-failed",
                    serde_json::json!({
                        "message": "设备身份损坏，已使用临时身份。请重新配对所有已信任设备。"
                    }),
                );
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            soundlink_lib::commands::tray::handle_close_requested(window, event);
        })
        .invoke_handler(tauri::generate_handler![
            soundlink_lib::commands::start_receiver,
            soundlink_lib::commands::stop_receiver,
            soundlink_lib::commands::get_pairing_code,
            soundlink_lib::commands::get_pairing_lock_status,
            soundlink_lib::commands::get_app_version,
            soundlink_lib::commands::get_log_path,
            soundlink_lib::commands::get_log_preview,
            soundlink_lib::commands::set_default_capture_source,
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
            soundlink_lib::commands::list_trusted_receivers,
            soundlink_lib::commands::remove_trusted_receiver,
            soundlink_lib::commands::connect_trusted_receiver,
            soundlink_lib::commands::quit_app,
            soundlink_lib::commands::minimize_to_tray,
            soundlink_lib::commands::show_main_window,
            soundlink_lib::commands::get_app_settings,
            soundlink_lib::commands::set_app_settings,
            soundlink_lib::commands::set_close_action,
            soundlink_lib::commands::set_auto_start,
            soundlink_lib::commands::get_auto_start,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// 安装 panic hook：panic 时把消息与调用栈写到 `%APPDATA%\soundlink\crash-<ts>.log`。
/// P0 安全红线修复（NF-01 B6）：原 `.expect(...)` panic 后无堆栈收集。
#[cfg(feature = "tauri_app")]
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let payload = info.payload();
        let msg = if let Some(s) = payload.downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.clone()
        } else {
            "<non-string panic payload>".to_string()
        };
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "<unknown>".into());
        let backtrace = std::backtrace::Backtrace::force_capture();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let dir = dirs::config_dir()
            .map(|mut p| {
                p.push("soundlink");
                let _ = std::fs::create_dir_all(&p);
                p
            })
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        let crash_path = dir.join(format!("crash-{}.log", timestamp));
        let log = format!(
            "SoundLink Crash Report\nTimestamp: {}\nMessage: {}\nLocation: {}\n\nBacktrace:\n{}\n",
            timestamp, msg, location, backtrace
        );
        if let Err(e) = std::fs::write(&crash_path, &log) {
            eprintln!("写入崩溃日志失败：{} -> {:?}", e, crash_path);
        } else {
            eprintln!("崩溃日志已写入：{:?}", crash_path);
        }
        // 调用默认 hook 保持默认行为（打印到 stderr）。
        default_hook(info);
    }));
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
