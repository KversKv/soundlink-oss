//! Tauri commands：桥接前端 UI 与 Rust Core。
//!
//! 仅在 `tauri_app` feature 启用时编译。命令：
//! start_receiver / stop_receiver / get_pairing_code /
//! list_output_devices / select_output_device / get_status /
//! list_trusted_devices / remove_trusted_device。
//!
//! 阶段 3：start_receiver 启动 mDNS 广播 + 控制服务器（TCP），
//! 真实发送端通过配对握手派生 audio_key 并启动 UDP 接收。
//!
//! 阶段 5：start_sender / stop_sender / get_sender_status / discover_receivers /
//! list_capture_sources / get_role / set_role。

#![cfg(feature = "tauri_app")]

use crate::audio::capture::{self, CaptureSource};
use crate::audio::jitter_buffer::JitterMode;
use crate::audio::output::OutputDeviceInfo;
use crate::config::{AppConfig, AudioParams};
use crate::constants::{DEFAULT_AUDIO_PORT, DEFAULT_CONTROL_PORT};
use crate::device::device_identity::DeviceIdentity;
use crate::network::control_server::ControlServer;
use crate::network::discovery::{DiscoveredReceiver, MdnsBroadcaster, MdnsBrowser};
use crate::pairing::{PairingCodeManager, TrustStore, TrustedDevice};
use crate::receiver::{ReceiverEngine, ReceiverStatus};
use crate::sender::{SenderEngine, SenderStatus};
use parking_lot::Mutex;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{Manager, State};

/// 应用角色。
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum Role {
    Receiver,
    Sender,
}

impl Default for Role {
    fn default() -> Self {
        Role::Receiver
    }
}

/// 应用共享状态。
pub struct AppState {
    pub engine: Arc<ReceiverEngine>,
    pub sender: Arc<SenderEngine>,
    pub pairing: Arc<PairingCodeManager>,
    pub identity: Arc<Mutex<DeviceIdentity>>,
    pub trust: Arc<Mutex<TrustStore>>,
    pub selected_device: Arc<Mutex<Option<usize>>>,
    pub control: Mutex<Option<ControlServer>>,
    pub mdns: Mutex<Option<MdnsBroadcaster>>,
    pub device_name: Mutex<String>,
    pub role: Mutex<Role>,
    pub config: Arc<Mutex<AppConfig>>,
    pub config_dir: PathBuf,
    /// 调试：是否开启音频 RAW Data 转储（来自 main.rs 的 DUMP_ENABLE）。
    pub dump_enable: bool,
    /// 设备身份加载是否失败（D5）：true 时 main.rs setup emit `identity-load-failed`。
    pub identity_load_failed: bool,
}

impl AppState {
    /// `debug`：DEBUG 模式（配对码固定 12345678）。
    /// `dump_enable`：音频各阶段 RAW Data 转储开关。
    pub fn new(debug: bool, dump_enable: bool) -> Self {
        let dir = config_dir();
        let mut identity_load_failed = false;
        let identity = DeviceIdentity::load_or_create(&dir).unwrap_or_else(|e| {
            tracing::warn!("设备身份加载失败：{}；用临时身份。", e);
            identity_load_failed = true;
            let mut csprng = rand::rngs::OsRng;
            let sk = ed25519_dalek::SigningKey::generate(&mut csprng);
            let temp_identity = DeviceIdentity {
                device_id: format!("pc-tmp-{:03x}", rand::random::<u32>() & 0xFFF),
                signing_key: sk,
            };
            // D5：尝试持久化临时身份，避免重启后身份变化导致已信任设备失效。
            if let Err(e) = temp_identity.try_persist_temp(&dir) {
                tracing::error!("临时身份持久化失败：{}；重启后身份将变化。", e);
            }
            temp_identity
        });
        let trust_path = dir.join("trust_store.json");
        let trust = TrustStore::load_or_create(trust_path).unwrap_or_else(|e| {
            tracing::warn!("信任存储加载失败：{}；用内存存储。", e);
            TrustStore::in_memory()
        });
        let trust = Arc::new(Mutex::new(trust));
        let config = AppConfig::load_or_default(&dir);
        let pairing = Arc::new(PairingCodeManager::with_debug(debug));
        if config.pairing_code_mode == "fixed" {
            if let Err(e) = pairing.set_fixed_code(config.fixed_pairing_code.clone()) {
                tracing::warn!("固定配对码配置无效：{}", e);
            }
        }
        let jitter_mode = parse_jitter_mode(&config.jitter_mode).unwrap_or(JitterMode::Balanced);
        let engine = Arc::new(ReceiverEngine::with_dump(dump_enable));
        engine.set_jitter_mode(jitter_mode);
        engine.set_volume(config.volume);
        let role = parse_role(&config.role).unwrap_or_default();
        Self {
            engine,
            sender: Arc::new(SenderEngine::with_trust(trust.clone(), dump_enable)),
            pairing,
            identity: Arc::new(Mutex::new(identity)),
            trust,
            selected_device: Arc::new(Mutex::new(config.default_output_device)),
            control: Mutex::new(None),
            mdns: Mutex::new(None),
            device_name: Mutex::new(config.device_name.clone()),
            role: Mutex::new(role),
            config: Arc::new(Mutex::new(config)),
            config_dir: dir,
            dump_enable,
            identity_load_failed,
        }
    }
}

fn config_dir() -> PathBuf {
    let mut p = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    p.push("soundlink");
    p
}

fn parse_role(role: &str) -> Option<Role> {
    match role {
        "receiver" => Some(Role::Receiver),
        "sender" => Some(Role::Sender),
        _ => None,
    }
}

fn role_as_str(role: Role) -> &'static str {
    match role {
        Role::Receiver => "receiver",
        Role::Sender => "sender",
    }
}

fn parse_jitter_mode(mode: &str) -> Option<JitterMode> {
    match mode {
        "low" => Some(JitterMode::Low),
        "balanced" => Some(JitterMode::Balanced),
        "stable" => Some(JitterMode::Stable),
        "auto" => Some(JitterMode::Auto),
        _ => None,
    }
}

fn save_config(state: &AppState) -> Result<(), String> {
    state.config.lock().save(&state.config_dir)
}

#[derive(Debug, Serialize)]
pub struct StartResult {
    pub pairing_code: String,
    pub control_port: u16,
    pub audio_port: u16,
    pub device_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PairingSettings {
    pub mode: String,
    pub fixed_code: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DesktopSettings {
    pub device_name: String,
    pub role: String,
    pub selected_device: Option<usize>,
    pub jitter_mode: String,
    pub volume: f32,
    pub pairing: PairingSettings,
    pub audio_params: AudioParams,
    pub last_receiver_addr: String,
    pub selected_capture_source: String,
}

/// 启动接收器：生成配对码、启动 mDNS 广播 + 控制服务器。
/// 真实发送端配对后由控制服务器自动启动 UDP 接收。
#[tauri::command]
pub async fn start_receiver(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<StartResult, String> {
    let s = state.inner();
    let code = s.pairing.issue();
    let device_id = s.identity.lock().device_id.clone();

    // 启动 mDNS 广播。
    {
        let mdns = MdnsBroadcaster::new();
        let device_name = s.device_name.lock().clone();
        mdns.start(
            &device_id,
            &device_name,
            None,
            DEFAULT_CONTROL_PORT,
            DEFAULT_AUDIO_PORT,
            true,
        )?;
        *s.mdns.lock() = Some(mdns);
    }

    // 启动控制服务器。
    {
        let control = ControlServer::with_config(
            s.engine.clone(),
            s.pairing.clone(),
            s.identity.clone(),
            s.trust.clone(),
            s.selected_device.clone(),
            s.device_name.lock().clone(),
            DEFAULT_AUDIO_PORT,
            Some(s.config.clone()),
            Some(s.config_dir.clone()),
            // D4：传入 AppHandle 以便配对锁定时 emit 事件给前端。
            Some(app),
        );
        let bind = format!("0.0.0.0:{}", DEFAULT_CONTROL_PORT);
        control.start(&bind).await?;
        *s.control.lock() = Some(control);
    }

    Ok(StartResult {
        pairing_code: code,
        control_port: DEFAULT_CONTROL_PORT,
        audio_port: DEFAULT_AUDIO_PORT,
        device_id,
    })
}

/// 停止接收器：停止控制服务器、mDNS 广播、UDP 接收。
#[tauri::command]
pub fn stop_receiver(state: State<'_, AppState>) -> Result<(), String> {
    let s = state.inner();
    if let Some(c) = s.control.lock().take() {
        c.stop();
    }
    if let Some(m) = s.mdns.lock().take() {
        m.stop();
    }
    s.engine.stop();
    Ok(())
}

/// 优雅退出清理（D3）：停止 sender（带 1s 超时）+ receiver + control + mDNS。
/// 在 quit_app 与 tray quit 路径调用，避免依赖 Drop 导致退出卡顿或端口残留。
pub async fn cleanup_before_quit(state: &AppState) {
    // 1. 停止 sender（带 1s 超时，避免卡死）
    let _ = tokio::time::timeout(std::time::Duration::from_secs(1), state.sender.stop()).await;
    // 2. 停止 receiver
    state.engine.stop();
    // 3. 停止 control server + mDNS
    if let Some(c) = state.control.lock().take() {
        c.stop();
    }
    if let Some(m) = state.mdns.lock().take() {
        m.stop();
    }
    // 4. 短暂等待端口释放
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
}

/// 获取/刷新配对码。
#[tauri::command]
pub fn get_pairing_code(state: State<'_, AppState>) -> Result<String, String> {
    Ok(state.inner().pairing.issue())
}

/// D4：查询当前配对锁定状态。返回 { is_locked, remaining_secs, attempts }。
#[derive(Debug, Serialize)]
pub struct PairingLockStatus {
    pub is_locked: bool,
    pub remaining_secs: u64,
    pub attempts: u32,
}

#[tauri::command]
pub fn get_pairing_lock_status(state: State<'_, AppState>) -> Result<PairingLockStatus, String> {
    let (is_locked, remaining_secs, attempts) = state.inner().pairing.lock_status();
    Ok(PairingLockStatus {
        is_locked,
        remaining_secs,
        attempts,
    })
}

/// E1：应用元信息（关于页用）。
#[derive(Debug, Serialize)]
pub struct AppVersionInfo {
    pub version: &'static str,
    pub name: &'static str,
    pub license: &'static str,
    pub repository: &'static str,
    pub build_date: &'static str,
}

/// 获取应用版本/许可证/构建日期/仓库链接（关于页用，E1）。
#[tauri::command]
pub fn get_app_version() -> Result<AppVersionInfo, String> {
    Ok(AppVersionInfo {
        version: env!("CARGO_PKG_VERSION"),
        name: env!("CARGO_PKG_NAME"),
        license: "MIT",
        repository: "https://github.com/KversKv/SoundLink",
        build_date: env!("BUILD_DATE", "unknown"),
    })
}

/// E4：日志目录路径（`%APPDATA%/soundlink/logs/`）。
#[tauri::command]
pub fn get_log_path() -> Result<String, String> {
    let dir = crate::logging::log_dir()
        .ok_or_else(|| "无法定位日志目录".to_string())?;
    Ok(dir.to_string_lossy().into_owned())
}

/// E4：读取最新日志文件尾部 max_lines 行（默认 200）。供设置页只读预览。
#[tauri::command]
pub fn get_log_preview(max_lines: Option<usize>) -> Result<String, String> {
    let dir = crate::logging::log_dir()
        .ok_or_else(|| "无法定位日志目录".to_string())?;
    // 找出目录下最新的 soundlink-*.log 文件。
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .map_err(|e| format!("读取日志目录失败：{}", e))?
        .filter_map(Result::ok)
        .filter_map(|e| {
            let p = e.path();
            let name = p.file_name()?.to_string_lossy().to_string();
            if name.starts_with("soundlink-") && name.ends_with(".log") {
                Some(p)
            } else {
                None
            }
        })
        .collect();
    entries.sort();
    let latest = entries
        .last()
        .ok_or_else(|| "暂无日志文件".to_string())?;
    let content = std::fs::read_to_string(latest)
        .map_err(|e| format!("读取日志文件失败：{}", e))?;
    let limit = max_lines.unwrap_or(200);
    let lines: Vec<&str> = content.lines().collect();
    let start = if lines.len() > limit { lines.len() - limit } else { 0 };
    Ok(lines[start..].join("\n"))
}

/// E4：设置默认采集源（持久化到 config.selected_capture_source）。
#[tauri::command]
pub fn set_default_capture_source(
    state: State<'_, AppState>,
    source: String,
) -> Result<(), String> {
    {
        let mut cfg = state.config.lock();
        cfg.selected_capture_source = source;
    }
    save_config(state.inner())
}

#[tauri::command]
pub fn get_desktop_settings(state: State<'_, AppState>) -> Result<DesktopSettings, String> {
    let s = state.inner();
    let cfg = s.config.lock().clone();
    Ok(DesktopSettings {
        device_name: s.device_name.lock().clone(),
        role: role_as_str(*s.role.lock()).into(),
        selected_device: *s.selected_device.lock(),
        jitter_mode: s.engine.jitter_mode().as_str().into(),
        volume: s.engine.volume(),
        pairing: PairingSettings {
            mode: cfg.pairing_code_mode,
            fixed_code: s.pairing.fixed_code().unwrap_or_default(),
        },
        audio_params: cfg.audio_params,
        last_receiver_addr: cfg.last_receiver_addr,
        selected_capture_source: cfg.selected_capture_source,
    })
}

#[tauri::command]
pub fn set_pairing_settings(
    state: State<'_, AppState>,
    mode: String,
    fixed_code: Option<String>,
) -> Result<PairingSettings, String> {
    let s = state.inner();
    match mode.as_str() {
        "random" => {
            s.pairing.set_fixed_code(None)?;
            {
                let mut cfg = s.config.lock();
                cfg.pairing_code_mode = "random".into();
                cfg.fixed_pairing_code = None;
            }
            save_config(s)?;
            Ok(PairingSettings {
                mode: "random".into(),
                fixed_code: String::new(),
            })
        }
        "fixed" => {
            let code = fixed_code.unwrap_or_default();
            s.pairing.set_fixed_code(Some(code.clone()))?;
            {
                let mut cfg = s.config.lock();
                cfg.pairing_code_mode = "fixed".into();
                cfg.fixed_pairing_code = Some(code.clone());
            }
            save_config(s)?;
            if s.pairing.current().is_some() {
                s.pairing.issue();
            }
            Ok(PairingSettings {
                mode: "fixed".into(),
                fixed_code: code,
            })
        }
        other => Err(format!("未知配对码模式：{}", other)),
    }
}

/// 列举输出设备。
#[tauri::command]
pub fn list_output_devices(_state: State<'_, AppState>) -> Result<Vec<OutputDeviceInfo>, String> {
    Ok(crate::device::audio_device::list_output_devices())
}

/// 选择输出设备（索引，对应 list_output_devices 的顺序）。
/// `index=None`（前端传 null）时清除选择，回退到系统默认设备。
/// 防御性设计：避免前端意外传 null 时反序列化失败阻断 onboarding 等流程。
#[tauri::command]
pub fn select_output_device(
    state: State<'_, AppState>,
    index: Option<usize>,
) -> Result<(), String> {
    let s = state.inner();
    *s.selected_device.lock() = index;
    s.config.lock().default_output_device = index;
    save_config(s)?;
    if s.engine.is_running() {
        match index {
            Some(i) => tracing::info!("输出设备切换：{}（下个流生效）", i),
            None => tracing::info!("输出设备已重置为默认"),
        }
    }
    Ok(())
}

/// 获取状态。
#[tauri::command]
pub fn get_status(state: State<'_, AppState>) -> Result<ReceiverStatus, String> {
    Ok(state.inner().engine.status())
}

/// 列举已信任设备（接收端视角：信任的发送端，host 为 None）。
#[tauri::command]
pub fn list_trusted_devices(state: State<'_, AppState>) -> Result<Vec<TrustedDevice>, String> {
    Ok(state
        .inner()
        .trust
        .lock()
        .list()
        .iter()
        .filter(|d| d.host.is_none())
        .cloned()
        .collect())
}

/// 移除已信任设备。
#[tauri::command]
pub fn remove_trusted_device(
    state: State<'_, AppState>,
    device_id: String,
) -> Result<bool, String> {
    state
        .inner()
        .trust
        .lock()
        .remove(&device_id)
        .map_err(|e| e.to_string())
}

/// 设置设备显示名（用于 mDNS 广播）。
#[tauri::command]
pub fn set_device_name(state: State<'_, AppState>, name: String) -> Result<(), String> {
    let s = state.inner();
    *s.device_name.lock() = name.clone();
    s.config.lock().device_name = name;
    save_config(s)
}

/// 切换 Jitter 模式（阶段 4）。
/// mode: "low" | "balanced" | "stable" | "auto"
#[tauri::command]
pub fn set_jitter_mode(state: State<'_, AppState>, mode: String) -> Result<String, String> {
    let m = parse_jitter_mode(&mode).ok_or_else(|| format!("未知 jitter 模式：{}", mode))?;
    let s = state.inner();
    s.engine.set_jitter_mode(m);
    {
        let mut cfg = s.config.lock();
        cfg.jitter_mode = m.as_str().into();
        cfg.audio_params.jitter_mode = m.as_str().into();
    }
    save_config(s)?;
    Ok(m.as_str().to_string())
}

/// 获取当前 Jitter 模式（阶段 4）。
#[tauri::command]
pub fn get_jitter_mode(state: State<'_, AppState>) -> Result<String, String> {
    Ok(state.inner().engine.jitter_mode().as_str().to_string())
}

/// 设置输出音量（阶段 4+）。`volume ∈ [0.0, 1.0]`。
#[tauri::command]
pub fn set_volume(state: State<'_, AppState>, volume: f32) -> Result<f32, String> {
    let s = state.inner();
    s.engine.set_volume(volume);
    let v = s.engine.volume();
    s.config.lock().volume = v;
    save_config(s)?;
    Ok(v)
}

#[tauri::command]
pub fn get_audio_params(state: State<'_, AppState>) -> Result<AudioParams, String> {
    Ok(state.inner().config.lock().audio_params.clone())
}

#[tauri::command]
pub fn set_audio_params(
    state: State<'_, AppState>,
    params: AudioParams,
) -> Result<AudioParams, String> {
    let s = state.inner();
    let params = params.normalized();
    if let Some(mode) = parse_jitter_mode(&params.jitter_mode) {
        s.engine.set_jitter_mode(mode);
    }
    {
        let mut cfg = s.config.lock();
        cfg.jitter_mode = params.jitter_mode.clone();
        cfg.audio_params = params.clone();
    }
    save_config(s)?;
    Ok(params)
}

#[tauri::command]
pub fn auto_detect_audio_params(state: State<'_, AppState>) -> Result<AudioParams, String> {
    let s = state.inner();
    let receiver = s.engine.status();
    let sender = s.sender.status();
    let mut params = s.config.lock().audio_params.clone().normalized();
    let loss_rate = receiver.loss_rate;
    let jitter_ms = receiver.jitter_ms;
    let recommended = if receiver.recommended_bitrate > 0 {
        receiver.recommended_bitrate
    } else {
        sender.recommended_bitrate
    };
    params.bitrate = if recommended > 0 {
        nearest_bitrate(recommended)
    } else if loss_rate >= 0.05 || jitter_ms >= 35 {
        96_000
    } else if loss_rate <= 0.01 && jitter_ms <= 12 {
        160_000
    } else {
        128_000
    };
    params.jitter_mode = if loss_rate >= 0.05 || jitter_ms >= 35 {
        "stable".into()
    } else if loss_rate <= 0.01 && jitter_ms <= 12 {
        "low".into()
    } else {
        "balanced".into()
    };
    set_audio_params(state, params)
}

fn nearest_bitrate(value: u32) -> u32 {
    [64_000u32, 96_000, 128_000, 160_000, 192_000]
        .into_iter()
        .min_by_key(|candidate| (*candidate).abs_diff(value))
        .unwrap_or(128_000)
}

/// 获取当前输出音量 `∈ [0.0, 1.0]`。
#[tauri::command]
pub fn get_volume(state: State<'_, AppState>) -> Result<f32, String> {
    Ok(state.inner().engine.volume())
}

// ───────────────────────── 阶段 5：桌面发送端 ─────────────────────────

/// 可用采集源信息。
#[derive(Debug, Serialize)]
pub struct CaptureSourceInfo {
    pub id: String,
    pub name: String,
    pub available: bool,
}

/// 列举可用采集源。
#[tauri::command]
pub fn list_capture_sources() -> Result<Vec<CaptureSourceInfo>, String> {
    #[allow(unused_mut)]
    let mut sources = vec![CaptureSourceInfo {
        id: "sine".into(),
        name: "440Hz 正弦测试源".into(),
        available: true,
    }];
    #[cfg(all(windows, feature = "wasapi"))]
    {
        sources.push(CaptureSourceInfo {
            id: "wasapi".into(),
            name: "WASAPI Loopback（系统音频）".into(),
            available: true,
        });
    }
    #[cfg(target_os = "macos")]
    {
        sources.push(CaptureSourceInfo {
            id: "screencapturekit".into(),
            name: "ScreenCaptureKit（未实现）".into(),
            available: false,
        });
    }
    Ok(sources)
}

/// 构造采集源（内部辅助）。
fn make_capture_source(source_id: &str) -> Result<Box<dyn CaptureSource>, String> {
    match source_id {
        "sine" | "" => Ok(capture::default_test_source()),
        #[cfg(all(windows, feature = "wasapi"))]
        "wasapi" => Ok(Box::new(
            capture::wasapi_loopback::WasapiLoopbackCapture::new(),
        )),
        other => Err(format!("未知采集源：{}", other)),
    }
}

/// 启动发送端：连接 Receiver → 握手 → 采集 → 发送。D1：启用 backoff 重连。
#[tauri::command]
pub async fn start_sender(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    receiver_addr: String,
    pairing_code: String,
    capture_source: Option<String>,
) -> Result<(), String> {
    use tauri::Emitter;
    let s = state.inner();
    let source_id = capture_source.unwrap_or_else(|| {
        #[cfg(all(windows, feature = "wasapi"))]
        {
            "wasapi".into()
        }
        #[cfg(not(all(windows, feature = "wasapi")))]
        {
            "sine".into()
        }
    });
    // D1：注入状态变化回调（首次注入后复用；重复调用会覆盖，但回调逻辑相同）。
    let app_for_cb = app.clone();
    s.sender.set_on_state_change(Box::new(move |state, error| {
        let _ = app_for_cb.emit(
            "sender-state-changed",
            serde_json::json!({ "state": state, "error": error }),
        );
    }));
    // I5：注入公钥不一致回调（公钥不匹配时仍 return Err，回调仅作 UI 告知）。
    let app_for_pkm = app.clone();
    s.sender.set_on_pubkey_mismatch(Box::new(move |device_id, device_name, saved_pub, recv_pub| {
        let _ = app_for_pkm.emit(
            "pubkey-mismatch",
            serde_json::json!({
                "device_id": device_id,
                "device_name": device_name,
                "saved_pub_b64": saved_pub,
                "recv_pub_b64": recv_pub,
            }),
        );
    }));
    // D1：capture_factory 闭包，重连时重新构造采集源。
    let source_id_for_factory = source_id.clone();
    let capture_factory: Arc<dyn Fn() -> Box<dyn CaptureSource> + Send + Sync> =
        Arc::new(move || make_capture_source(&source_id_for_factory).unwrap_or_else(|_| capture::default_test_source()));
    let (device_id, device_name, signing_key) = {
        let id = s.identity.lock();
        (
            id.device_id.clone(),
            s.device_name.lock().clone(),
            id.signing_key.clone(),
        )
    };
    let audio_params = s.config.lock().audio_params.clone().normalized();
    {
        let mut cfg = s.config.lock();
        cfg.last_receiver_addr = receiver_addr.clone();
        cfg.selected_capture_source = source_id;
    }
    save_config(s)?;
    s.sender
        .start_with_reconnect(
            capture_factory,
            &receiver_addr,
            &pairing_code,
            &device_id,
            &device_name,
            &signing_key,
            DEFAULT_AUDIO_PORT,
            audio_params,
        )
        .await
}

/// 停止发送端。
#[tauri::command]
pub async fn stop_sender(state: State<'_, AppState>) -> Result<(), String> {
    state.inner().sender.stop().await;
    Ok(())
}

/// 获取发送端状态。
#[tauri::command]
pub fn get_sender_status(state: State<'_, AppState>) -> Result<SenderStatus, String> {
    Ok(state.inner().sender.status())
}

/// 发现局域网内的 Receiver（mDNS 浏览）。
#[tauri::command]
pub async fn discover_receivers(
    duration_secs: Option<u64>,
) -> Result<Vec<DiscoveredReceiver>, String> {
    let browser = MdnsBrowser::new();
    browser.browse(duration_secs.unwrap_or(2))
}

/// 获取当前角色。
#[tauri::command]
pub fn get_role(state: State<'_, AppState>) -> Result<String, String> {
    Ok(match *state.inner().role.lock() {
        Role::Receiver => "receiver".into(),
        Role::Sender => "sender".into(),
    })
}

/// 切换角色。
#[tauri::command]
pub fn set_role(state: State<'_, AppState>, role: String) -> Result<String, String> {
    let r = parse_role(&role).ok_or_else(|| format!("未知角色：{}", role))?;
    let s = state.inner();
    *s.role.lock() = r;
    s.config.lock().role = role_as_str(r).into();
    save_config(s)?;
    Ok(role_as_str(r).into())
}

// ─────────────────── 发送端：记住设备 / 信任直连 ───────────────────

/// 列举已信任的 Receiver（发送端视角：host 为 Some 的条目）。
#[tauri::command]
pub fn list_trusted_receivers(state: State<'_, AppState>) -> Result<Vec<TrustedDevice>, String> {
    Ok(state
        .inner()
        .trust
        .lock()
        .list()
        .iter()
        .filter(|d| d.host.is_some())
        .cloned()
        .collect())
}

/// 移除已信任的 Receiver。
#[tauri::command]
pub fn remove_trusted_receiver(
    state: State<'_, AppState>,
    device_id: String,
) -> Result<bool, String> {
    state
        .inner()
        .trust
        .lock()
        .remove(&device_id)
        .map_err(|e| e.to_string())
}

/// 一键直连已信任的 Receiver。
///
/// 从信任存储中按 `device_id` 查找设备，取出 host/control_port 拼接地址，
/// 配对码留空（走已信任路径），调用与 `start_sender` 相同的启动逻辑。
#[tauri::command]
pub async fn connect_trusted_receiver(
    state: State<'_, AppState>,
    device_id: String,
    capture_source: Option<String>,
) -> Result<(), String> {
    let s = state.inner();
    // 查找设备并提取连接信息。
    let receiver_addr = {
        let trust = s.trust.lock();
        let dev = trust
            .get(&device_id)
            .ok_or_else(|| format!("未找到已信任设备：{}", device_id))?;
        let host = dev.host.clone().ok_or_else(|| "设备缺少 host 信息".to_string())?;
        let control_port = dev
            .control_port
            .ok_or_else(|| "设备缺少 control_port 信息".to_string())?;
        format!("{}:{}", host, control_port)
    };

    let source_id = capture_source.unwrap_or_else(|| {
        #[cfg(all(windows, feature = "wasapi"))]
        {
            "wasapi".into()
        }
        #[cfg(not(all(windows, feature = "wasapi")))]
        {
            "sine".into()
        }
    });
    let source = make_capture_source(&source_id)?;
    let (sender_device_id, sender_device_name, signing_key) = {
        let id = s.identity.lock();
        (
            id.device_id.clone(),
            s.device_name.lock().clone(),
            id.signing_key.clone(),
        )
    };
    let audio_params = s.config.lock().audio_params.clone().normalized();
    {
        let mut cfg = s.config.lock();
        cfg.last_receiver_addr = receiver_addr.clone();
        cfg.selected_capture_source = source_id;
    }
    save_config(s)?;
    // 配对码留空：走已信任路径（Receiver 端会识别本机身份）。
    s.sender
        .start(
            source,
            &receiver_addr,
            "",
            &sender_device_id,
            &sender_device_name,
            &signing_key,
            DEFAULT_AUDIO_PORT,
            audio_params,
        )
        .await
}

// ────────────────────────────────────────────────────────────────────
//  系统托盘 / 关闭窗口行为 / 设置面板
// ────────────────────────────────────────────────────────────────────

pub mod tray;

/// 设置面板专用：返回 `close_action` / `auto_start` / 自动收发开关。
#[derive(Debug, Clone, Serialize)]
pub struct AppSettings {
    pub close_action: String,
    pub auto_start: bool,
    pub auto_receive_on_start: bool,
    pub auto_send_on_start: bool,
    /// E3：是否已完成首次引导。
    pub onboarding_completed: bool,
    /// F6：发送端 DRM 提示是否已展示。
    pub sender_drm_hint_seen: bool,
}

/// 退出整个应用（非仅关闭窗口）。D3：先调 cleanup_before_quit 再 exit。
#[tauri::command]
pub async fn quit_app(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    cleanup_before_quit(state.inner()).await;
    app.exit(0);
    Ok(())
}

/// 最小化到托盘：隐藏主窗口。
#[tauri::command]
pub fn minimize_to_tray(app: tauri::AppHandle) -> Result<(), String> {
    let w = app
        .get_webview_window("main")
        .ok_or_else(|| "主窗口未找到".to_string())?;
    w.hide().map_err(|e| e.to_string())
}

/// 显示主窗口并聚焦。
#[tauri::command]
pub fn show_main_window(app: tauri::AppHandle) -> Result<(), String> {
    let w = app
        .get_webview_window("main")
        .ok_or_else(|| "主窗口未找到".to_string())?;
    w.show().map_err(|e| e.to_string())?;
    w.set_focus().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_app_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    let cfg = state.config.lock();
    Ok(AppSettings {
        close_action: cfg.close_action.clone(),
        auto_start: cfg.auto_start,
        auto_receive_on_start: cfg.auto_receive_on_start,
        auto_send_on_start: cfg.auto_send_on_start,
        onboarding_completed: cfg.onboarding_completed,
        sender_drm_hint_seen: cfg.sender_drm_hint_seen,
    })
}

/// 批量保存设置：写入 config + 同步 autostart 注册表项（仅当 `auto_start` 被显式设置）。
#[tauri::command]
pub async fn set_app_settings(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    close_action: Option<String>,
    auto_start: Option<bool>,
    auto_receive_on_start: Option<bool>,
    auto_send_on_start: Option<bool>,
    onboarding_completed: Option<bool>,
    sender_drm_hint_seen: Option<bool>,
) -> Result<AppSettings, String> {
    {
        let mut cfg = state.config.lock();
        if let Some(v) = close_action {
            if !["ask", "minimize", "quit"].contains(&v.as_str()) {
                return Err(format!("非法 close_action：{}", v));
            }
            cfg.close_action = v;
        }
        if let Some(v) = auto_start {
            cfg.auto_start = v;
        }
        if let Some(v) = auto_receive_on_start {
            cfg.auto_receive_on_start = v;
        }
        if let Some(v) = auto_send_on_start {
            cfg.auto_send_on_start = v;
        }
        if let Some(v) = onboarding_completed {
            cfg.onboarding_completed = v;
        }
        if let Some(v) = sender_drm_hint_seen {
            cfg.sender_drm_hint_seen = v;
        }
    }
    save_config(state.inner())?;
    if let Some(v) = auto_start {
        sync_autostart(&app, v)?;
    }
    get_app_settings(state)
}

/// 仅设置关闭行为（关闭对话框「记住选择」时调用）。
#[tauri::command]
pub fn set_close_action(state: State<'_, AppState>, action: String) -> Result<(), String> {
    if !["ask", "minimize", "quit"].contains(&action.as_str()) {
        return Err(format!("非法 close_action：{}", action));
    }
    state.config.lock().close_action = action;
    save_config(state.inner())
}

#[tauri::command]
pub fn set_auto_start(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<bool, String> {
    state.config.lock().auto_start = enabled;
    save_config(state.inner())?;
    sync_autostart(&app, enabled)?;
    Ok(enabled)
}

#[tauri::command]
pub fn get_auto_start(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.config.lock().auto_start)
}

/// 同步 autostart 插件注册表项与配置中的 `auto_start` 字段一致。
fn sync_autostart(app: &tauri::AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let mgr = app.autolaunch();
    if enabled {
        mgr.enable().map_err(|e| format!("启用自启动失败：{}", e))
    } else {
        let _ = mgr.disable();
        Ok(())
    }
}

#[cfg(all(test, feature = "tauri_app"))]
mod tests {
    use super::*;

    #[test]
    fn parse_role_valid() {
        assert_eq!(parse_role("receiver"), Some(Role::Receiver));
        assert_eq!(parse_role("sender"), Some(Role::Sender));
    }

    #[test]
    fn parse_role_invalid() {
        assert_eq!(parse_role("foo"), None);
        assert_eq!(parse_role(""), None);
    }

    #[test]
    fn role_as_str_roundtrip() {
        assert_eq!(role_as_str(Role::Receiver), "receiver");
        assert_eq!(role_as_str(Role::Sender), "sender");
    }

    #[test]
    fn parse_jitter_mode_all_variants() {
        assert_eq!(parse_jitter_mode("low"), Some(JitterMode::Low));
        assert_eq!(parse_jitter_mode("balanced"), Some(JitterMode::Balanced));
        assert_eq!(parse_jitter_mode("stable"), Some(JitterMode::Stable));
        assert_eq!(parse_jitter_mode("auto"), Some(JitterMode::Auto));
    }

    #[test]
    fn parse_jitter_mode_invalid() {
        assert_eq!(parse_jitter_mode("foo"), None);
        assert_eq!(parse_jitter_mode(""), None);
    }

    #[test]
    fn nearest_bitrate_exact() {
        assert_eq!(nearest_bitrate(128_000), 128_000);
        assert_eq!(nearest_bitrate(64_000), 64_000);
        assert_eq!(nearest_bitrate(192_000), 192_000);
    }

    #[test]
    fn nearest_bitrate_round_down() {
        // 70000 距 64000(6000) 比 96000(26000) 更近
        assert_eq!(nearest_bitrate(70_000), 64_000);
    }

    #[test]
    fn nearest_bitrate_round_up() {
        // 110000 距 96000(14000) 比 128000(18000) 更近
        assert_eq!(nearest_bitrate(110_000), 96_000);
    }

    #[test]
    fn nearest_bitrate_above_max_clamps() {
        assert_eq!(nearest_bitrate(999_999), 192_000);
    }

    #[test]
    fn nearest_bitrate_below_min_clamps() {
        assert_eq!(nearest_bitrate(1_000), 64_000);
    }

    #[test]
    fn nearest_bitrate_midpoint_picks_lower() {
        // 112000 是 96000 与 128000 的中点，min_by_key 平局取首个（96000）
        assert_eq!(nearest_bitrate(112_000), 96_000);
    }

    #[test]
    fn make_capture_source_sine_returns_default_test_source() {
        let mut src = make_capture_source("sine").unwrap();
        assert!(!src.name().is_empty());
        assert!(src.start().is_ok());
        let frame = src.poll_frame();
        assert!(frame.is_some());
        assert!(!frame.unwrap().is_empty());
        src.stop();
    }

    #[test]
    fn make_capture_source_empty_falls_back_to_sine() {
        let src = make_capture_source("").unwrap();
        assert!(!src.name().is_empty());
    }

    #[test]
    fn make_capture_source_unknown_returns_err() {
        let r = make_capture_source("foobar");
        assert!(r.is_err());
        let e = r.err().unwrap();
        assert!(e.contains("未知采集源"), "错误信息应含「未知采集源」，got: {}", e);
    }
}
