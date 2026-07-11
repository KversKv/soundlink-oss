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
use tauri::State;

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
}

impl AppState {
    /// `debug`：DEBUG 模式（配对码固定 12345678）。
    /// `dump_enable`：音频各阶段 RAW Data 转储开关。
    pub fn new(debug: bool, dump_enable: bool) -> Self {
        let dir = config_dir();
        let identity = DeviceIdentity::load_or_create(&dir).unwrap_or_else(|e| {
            tracing::warn!("设备身份加载失败：{}；用临时身份。", e);
            let mut csprng = rand::rngs::OsRng;
            let sk = ed25519_dalek::SigningKey::generate(&mut csprng);
            DeviceIdentity {
                device_id: format!("pc-tmp-{:03x}", rand::random::<u32>() & 0xFFF),
                signing_key: sk,
            }
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
pub async fn start_receiver(state: State<'_, AppState>) -> Result<StartResult, String> {
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

/// 获取/刷新配对码。
#[tauri::command]
pub fn get_pairing_code(state: State<'_, AppState>) -> Result<String, String> {
    Ok(state.inner().pairing.issue())
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
#[tauri::command]
pub fn select_output_device(state: State<'_, AppState>, index: usize) -> Result<(), String> {
    let s = state.inner();
    *s.selected_device.lock() = Some(index);
    s.config.lock().default_output_device = Some(index);
    save_config(s)?;
    if s.engine.is_running() {
        tracing::info!("输出设备切换：{}（下个流生效）", index);
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

/// 启动发送端：连接 Receiver → 握手 → 采集 → 发送。
#[tauri::command]
pub async fn start_sender(
    state: State<'_, AppState>,
    receiver_addr: String,
    pairing_code: String,
    capture_source: Option<String>,
) -> Result<(), String> {
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
    let source = make_capture_source(&source_id)?;
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
        .start(
            source,
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
