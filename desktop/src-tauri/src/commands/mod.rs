//! Tauri commands：桥接前端 UI 与 Rust Core（spec §8.1 最小集）。
//!
//! 仅在 `tauri_app` feature 启用时编译。命令：
//! start_receiver / stop_receiver / get_pairing_code /
//! list_output_devices / select_output_device / get_status。
//!
//! 阶段 1：start_receiver 用自握手派生 audio_key（无远程发送端时仍可启动接收）。
//! 真实配对/控制通道在阶段 3 接入。

#![cfg(feature = "tauri_app")]

use crate::audio::output::OutputDeviceInfo;
use crate::constants::DEFAULT_AUDIO_PORT;
use crate::device::device_identity::DeviceIdentity;
use crate::pairing::{
    derive_pairing_secret, derive_session_keys, diffie_hellman, EphemeralKeyPair,
    PairingCodeManager,
};
use crate::receiver::{ReceiverEngine, ReceiverStatus};
use parking_lot::Mutex;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;

/// 应用共享状态。
pub struct AppState {
    pub engine: Arc<ReceiverEngine>,
    pub pairing: PairingCodeManager,
    pub identity: Mutex<DeviceIdentity>,
    pub selected_device: Mutex<Option<usize>>,
    /// 当前会话 audio_key（自握手派生，阶段 3 由配对流程产出）。
    pub audio_key: Mutex<Option<[u8; 32]>>,
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
        Self {
            engine: Arc::new(ReceiverEngine::new()),
            pairing: PairingCodeManager::new(),
            identity: Mutex::new(identity),
            selected_device: Mutex::new(None),
            audio_key: Mutex::new(None),
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
    pub audio_port: u16,
    pub device_id: String,
}

/// 启动接收器：生成配对码、自握手派生 audio_key、绑定 UDP、起 cpal 输出。
#[tauri::command]
pub async fn start_receiver(state: State<'_, AppState>) -> Result<StartResult, String> {
    let code = state.pairing.issue();
    let device_id = state.identity.lock().device_id.clone();
    let pairing_secret = derive_pairing_secret(&code, &device_id);

    // 阶段 1 自握手：接收端生成 X25519 密钥对，自己充当对端，派生 audio_key。
    // （真实配对在阶段 3：sender 发 pair_request，控制通道交换公钥。）
    let recv_kp = EphemeralKeyPair::generate();
    let send_kp = EphemeralKeyPair::generate();
    let shared = diffie_hellman(recv_kp.secret, &send_kp.public);
    let keys = derive_session_keys(&shared, &pairing_secret);
    *state.audio_key.lock() = Some(keys.audio_key);

    let dev = *state.selected_device.lock();
    let bind = format!("0.0.0.0:{}", DEFAULT_AUDIO_PORT);
    state
        .engine
        .start(
            keys.audio_key,
            crate::constants::DEFAULT_STREAM_ID,
            &bind,
            dev,
        )
        .await
        .map_err(|e| e.to_string())?;

    Ok(StartResult {
        pairing_code: code,
        audio_port: DEFAULT_AUDIO_PORT,
        device_id,
    })
}

/// 停止接收器。
#[tauri::command]
pub fn stop_receiver(state: State<'_, AppState>) -> Result<(), String> {
    state.engine.stop();
    *state.audio_key.lock() = None;
    Ok(())
}

/// 获取/刷新配对码。
#[tauri::command]
pub fn get_pairing_code(state: State<'_, AppState>) -> Result<String, String> {
    Ok(state.pairing.issue())
}

/// 列举输出设备。
#[tauri::command]
pub fn list_output_devices(_state: State<'_, AppState>) -> Result<Vec<OutputDeviceInfo>, String> {
    Ok(crate::device::audio_device::list_output_devices())
}

/// 选择输出设备（索引，对应 list_output_devices 的顺序）。
#[tauri::command]
pub fn select_output_device(state: State<'_, AppState>, index: usize) -> Result<(), String> {
    *state.selected_device.lock() = Some(index);
    // 若接收器在运行，需要重启以应用设备。
    if state.engine.is_running() {
        tracing::info!("输出设备切换：{}", index);
    }
    Ok(())
}

/// 获取状态。
#[tauri::command]
pub fn get_status(state: State<'_, AppState>) -> Result<ReceiverStatus, String> {
    Ok(state.engine.status())
}
