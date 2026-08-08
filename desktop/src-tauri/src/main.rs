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
    // QR-1：主程序被复制/重命名为 qr_helper.exe 时进入提权辅助进程模式
    // （便携单文件形态自举，见 helper_client::install_helper）。
    // 必须先于一切插件/Tauri 初始化分发，避免单实例锁把 --serve 请求吞掉。
    #[cfg(windows)]
    {
        let helper_mode = std::env::current_exe()
            .ok()
            .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
            .is_some_and(|s| s.eq_ignore_ascii_case("qr_helper"));
        if helper_mode {
            let args: Vec<String> = std::env::args().collect();
            std::process::exit(soundlink_lib::features::quick_resolution::helper_core::run(&args));
        }
    }
    soundlink_lib::logging::init();
    install_panic_hook();
    // MON-01 S5：系统自启动拉起标记（autostart 插件注册的启动参数）。
    let autostarted = std::env::args().any(|a| a == "--autostarted");
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
        // MON-01 S13：全局快捷键处理器。事件 kind 由 ShortcutAction 映射，
        // 具体动作在前端（隐藏窗口下 webview 仍在运行，事件照常分发）。
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        use tauri::Emitter;
                        let kind = shortcut_action_kind(app, &shortcut.to_string());
                        if let Some(kind) = kind {
                            let _ = app.emit("global-shortcut", serde_json::json!({ "kind": kind }));
                        }
                    }
                })
                .build(),
        )
        .manage(soundlink_lib::commands::AppState::new(DEBUG, DUMP_ENABLE, autostarted))
        .setup(|app| {
            if let Err(e) = soundlink_lib::commands::tray::setup_tray(app) {
                tracing::warn!("托盘初始化失败：{}（应用继续启动）", e);
            }
            let state: tauri::State<soundlink_lib::commands::AppState> = app.state();

            // MON-01 S13/S14：快捷键改为能力驱动（免费仅 Ctrl+Shift+S 显示主窗口）。
            // 注册失败不再只 warn：汇总后 emit 事件给前端提示（S14 冲突检测）。
            {
                use tauri_plugin_global_shortcut::GlobalShortcutExt;
                let custom = state.config.lock().shortcuts.clone();
                let bindings = state.caps.shortcuts(&custom);
                let mut failed: Vec<String> = Vec::new();
                for b in &bindings {
                    if let Err(e) = app.global_shortcut().register(b.accelerator.as_str()) {
                        tracing::warn!("注册全局快捷键 {} 失败：{}", b.accelerator, e);
                        failed.push(b.accelerator.clone());
                    }
                }
                if !failed.is_empty() {
                    use tauri::Emitter;
                    let _ = app.emit(
                        "shortcut-register-failed",
                        serde_json::json!({ "accelerators": failed }),
                    );
                }
            }

            // MON-01 S1：发送端信任条目被容量上限替换时注入回调（emit 事件提示前端）。
            {
                let app_handle = app.handle().clone();
                state.sender.set_on_trust_evicted(Box::new(move |old, max| {
                    use tauri::Emitter;
                    let _ = app_handle.emit(
                        "trust-device-evicted",
                        serde_json::json!({
                            "device_id": old.device_id,
                            "name": old.name,
                            "max": max,
                        }),
                    );
                }));
            }

            // MON-01 S5：静默启动 —— 启动计划要求 silent 时保持窗口隐藏（最小化到托盘）。
            // tauri.conf.json 主窗口保持 visible: true；免费版 startup_plan 恒 None，行为不变。
            if let Some(plan) = state.startup_plan() {
                if plan.silent {
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = w.hide();
                    }
                    tracing::info!("静默启动：窗口保持隐藏（最小化到托盘）");
                }
            }

            // D5：identity 加载失败时通知前端提示用户重新配对。
            if state.identity_load_failed {
                use tauri::Emitter;
                let _ = app.emit(
                    "identity-load-failed",
                    serde_json::json!({
                        "message": "设备身份损坏，已使用临时身份。请重新配对所有已信任设备。"
                    }),
                );
            }

            // QR-1：显示器热插拔监听（Windows 生效）+ 启动状态自检（Stale 标记）。
            {
                use tauri::Listener;
                soundlink_lib::features::quick_resolution::platform::start_display_hook(
                    app.handle().clone(),
                );
                let app_handle = app.handle().clone();
                app.listen_any("qr://display-changed", move |_| {
                    let st: tauri::State<soundlink_lib::commands::AppState> = app_handle.state();
                    st.qr.refresh_states(&app_handle);
                    soundlink_lib::commands::tray::refresh_tray(&app_handle);
                });
                let app_handle = app.handle().clone();
                let st: tauri::State<soundlink_lib::commands::AppState> = app.state();
                let svc = st.qr.clone();
                tauri::async_runtime::spawn(async move {
                    svc.refresh_states(&app_handle);
                });
            }

            // QR-1 L2 启动自检：上次预置未收尾 → 回滚 EDID。
            {
                let st: tauri::State<soundlink_lib::commands::AppState> = app.state();
                if let Some(backup_id) = st.qr.startup_recovery() {
                    tracing::warn!("QR 启动自检：已回滚未收尾预置（backup={}）", backup_id);
                }
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
            soundlink_lib::commands::get_local_addresses,
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
            soundlink_lib::commands::get_license_status,
            soundlink_lib::commands::activate_license,
            soundlink_lib::commands::deactivate_license,
            soundlink_lib::commands::resolve_startup_plan,
            soundlink_lib::commands::list_profiles,
            soundlink_lib::commands::save_profile,
            soundlink_lib::commands::apply_profile,
            soundlink_lib::commands::delete_profile,
            soundlink_lib::commands::rename_profile,
            soundlink_lib::commands::get_shortcuts,
            soundlink_lib::commands::get_tray_state,
            soundlink_lib::commands::toggle_mute,
            soundlink_lib::commands::get_trust_quota,
            soundlink_lib::commands::resolve_auto_send_target,
            // QR-1 分辨率快速切换（Pro；命令内部以能力值门控）
            soundlink_lib::features::quick_resolution::commands::qr_get_availability,
            soundlink_lib::features::quick_resolution::commands::qr_get_displays,
            soundlink_lib::features::quick_resolution::commands::qr_identify_displays,
            soundlink_lib::features::quick_resolution::commands::qr_get_settings,
            soundlink_lib::features::quick_resolution::commands::qr_set_settings,
            soundlink_lib::features::quick_resolution::commands::qr_list_modes,
            soundlink_lib::features::quick_resolution::commands::qr_upsert_mode,
            soundlink_lib::features::quick_resolution::commands::qr_delete_mode,
            soundlink_lib::features::quick_resolution::commands::qr_reorder_modes,
            soundlink_lib::features::quick_resolution::commands::qr_import_system_modes,
            soundlink_lib::features::quick_resolution::commands::qr_validate_mode,
            soundlink_lib::features::quick_resolution::commands::qr_apply,
            soundlink_lib::features::quick_resolution::commands::qr_apply_previous,
            soundlink_lib::features::quick_resolution::commands::qr_confirm_apply,
            soundlink_lib::features::quick_resolution::commands::qr_revert_apply,
            soundlink_lib::features::quick_resolution::commands::qr_list_edid_backups,
            soundlink_lib::features::quick_resolution::commands::qr_refresh_states,
            soundlink_lib::features::quick_resolution::commands::qr_get_dsc_status,
            soundlink_lib::features::quick_resolution::commands::qr_install_helper,
            soundlink_lib::features::quick_resolution::commands::qr_helper_status,
            soundlink_lib::features::quick_resolution::commands::qr_provision,
            soundlink_lib::features::quick_resolution::commands::qr_export_diagnostics,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// MON-01 S13：把快捷键 accelerator 映射为事件 kind（按当前能力集反查动作）。
#[cfg(feature = "tauri_app")]
fn shortcut_action_kind(app: &tauri::AppHandle, accelerator: &str) -> Option<&'static str> {
    use soundlink_pro_api::ShortcutAction;
    use tauri::Manager;
    let state: tauri::State<soundlink_lib::commands::AppState> = app.state();
    let custom = state.config.lock().shortcuts.clone();
    let bindings = state.caps.shortcuts(&custom);
    let action = bindings
        .iter()
        .find(|b| b.accelerator == accelerator)?
        .action;
    Some(match action {
        ShortcutAction::ShowWindow => "show-window",
        ShortcutAction::ToggleRole => "toggle-role",
        ShortcutAction::StartStopReceiver => "start-stop-receiver",
        ShortcutAction::StartStopSender => "start-stop-sender",
        ShortcutAction::CycleOutputDevice => "cycle-output-device",
        ShortcutAction::ToggleMute => "toggle-mute",
    })
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
