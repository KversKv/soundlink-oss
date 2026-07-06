//! Tauri commands：桥接前端 UI 与 Rust Core。
//!
//! 仅在 `tauri_app` feature 启用时编译。命令：
//! start_receiver / stop_receiver / get_pairing_code /
//! list_output_devices / select_output_device / get_status /
//! list_trusted_devices / remove_trusted_device。
//!
//! 阶段 3：start_receiver 启动 mDNS 广播 + 控制服务器（TCP），
//! 真实发送端通过配对握手派生 audio_key 并启动 UDP 接收。

#![cfg(feature = "tauri_app")]

use crate::audio::output::OutputDeviceInfo;
use crate::constants::{DEFAULT_AUDIO_PORT, DEFAULT_CONTROL_PORT};
use crate::device::device_identity::DeviceIdentity;
use crate::network::control_server::ControlServer;
use crate::network::discovery::MdnsBroadcaster;
use crate::pairing::{PairingCodeManager, TrustStore, TrustedDevice};
use crate::receiver::{ReceiverEngine, ReceiverStatus};
use parking_lot::Mutex;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;

/// 应用共享状态。
pub struct AppState {
    pub engine: Arc<ReceiverEngine>,
    pub pairing: Arc<PairingCodeManager>,
    pub identity: Arc<Mutex<DeviceIdentity>>,
    pub trust: Arc<Mutex<TrustStore>>,
    pub selected_device: Arc<Mutex<Option<usize>>>,
    pub control: Mutex<Option<ControlServer>>,
    pub mdns: Mutex<Option<MdnsBroadcaster>>,
    pub device_name: Mutex<String>,
}

impl AppState {
    pub fn new() -> Self {
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
            engine: Arc::new(ReceiverEngine::new()),
            pairing: Arc::new(PairingCodeManager::new()),
            identity: Arc::new(Mutex::new(identity)),
            trust: Arc::new(Mutex::new(trust)),
            selected_device: Arc::new(Mutex::new(None)),
            control: Mutex::new(None),
            mdns: Mutex::new(None),
            device_name: Mutex::new("SoundLink Receiver".to_string()),
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
