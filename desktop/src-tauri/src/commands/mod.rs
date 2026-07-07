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
        Self {
            engine: Arc::new(ReceiverEngine::with_dump(dump_enable)),
            sender: Arc::new(SenderEngine::with_dump(dump_enable)),
            pairing: Arc::new(PairingCodeManager::with_debug(debug)),
            identity: Arc::new(Mutex::new(identity)),
            trust: Arc::new(Mutex::new(trust)),
            selected_device: Arc::new(Mutex::new(None)),
            control: Mutex::new(None),
            mdns: Mutex::new(None),
            device_name: Mutex::new("SoundLink Receiver".to_string()),
            role: Mutex::new(Role::default()),
            dump_enable,
        }
    }
}

fn config_dir() -> PathBuf {
    let mut p = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    p.push("soundlink");
    p
}

#[derive(Debug, Serialize)]
pub struct StartResult {
    pub pairing_code: String,
    pub control_port: u16,
    pub audio_port: u16,
    pub device_id: String,
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
        let control = ControlServer::new(
            s.engine.clone(),
            s.pairing.clone(),
            s.identity.clone(),
            s.trust.clone(),
            s.selected_device.clone(),
            s.device_name.lock().clone(),
            DEFAULT_AUDIO_PORT,
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

/// 列举输出设备。
#[tauri::command]
pub fn list_output_devices(_state: State<'_, AppState>) -> Result<Vec<OutputDeviceInfo>, String> {
    Ok(crate::device::audio_device::list_output_devices())
}

/// 选择输出设备（索引，对应 list_output_devices 的顺序）。
#[tauri::command]
pub fn select_output_device(state: State<'_, AppState>, index: usize) -> Result<(), String> {
    *state.inner().selected_device.lock() = Some(index);
    if state.inner().engine.is_running() {
        tracing::info!("输出设备切换：{}（下个流生效）", index);
    }
    Ok(())
}

/// 获取状态。
#[tauri::command]
pub fn get_status(state: State<'_, AppState>) -> Result<ReceiverStatus, String> {
    Ok(state.inner().engine.status())
}

/// 列举已信任设备。
#[tauri::command]
pub fn list_trusted_devices(state: State<'_, AppState>) -> Result<Vec<TrustedDevice>, String> {
    Ok(state.inner().trust.lock().list().to_vec())
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
    *state.inner().device_name.lock() = name;
    Ok(())
}

/// 切换 Jitter 模式（阶段 4）。
/// mode: "low" | "balanced" | "stable" | "auto"
#[tauri::command]
pub fn set_jitter_mode(state: State<'_, AppState>, mode: String) -> Result<String, String> {
    let m = match mode.as_str() {
        "low" => JitterMode::Low,
        "balanced" => JitterMode::Balanced,
        "stable" => JitterMode::Stable,
        "auto" => JitterMode::Auto,
        other => return Err(format!("未知 jitter 模式：{}", other)),
    };
    state.inner().engine.set_jitter_mode(m);
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
    state.inner().engine.set_volume(volume);
    Ok(state.inner().engine.volume())
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
    s.sender
        .start(
            source,
            &receiver_addr,
            &pairing_code,
            &device_id,
            &device_name,
            &signing_key,
            DEFAULT_AUDIO_PORT,
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
    let r = match role.as_str() {
        "receiver" => Role::Receiver,
        "sender" => Role::Sender,
        other => return Err(format!("未知角色：{}", other)),
    };
    *state.inner().role.lock() = r;
    Ok(match r {
        Role::Receiver => "receiver".into(),
        Role::Sender => "sender".into(),
    })
}
