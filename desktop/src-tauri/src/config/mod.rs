use crate::constants::{
    DEFAULT_AUDIO_PORT, DEFAULT_JITTER_MS, FRAME_DURATION_MS, JITTER_BALANCED_MS, OPUS_BITRATE,
    SAMPLE_RATE,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// OS keyring 服务名。
const KEYRING_SERVICE: &str = "soundlink";
/// 固定配对码在 keyring 中的账号名。
const KEYRING_ACCOUNT_FIXED_CODE: &str = "fixed_pairing_code";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioParams {
    pub sample_rate: u32,
    pub channels: u8,
    pub frame_duration_ms: u8,
    pub bitrate: u32,
    pub jitter_mode: String,
}

impl Default for AudioParams {
    fn default() -> Self {
        Self {
            sample_rate: SAMPLE_RATE,
            channels: 2,
            frame_duration_ms: FRAME_DURATION_MS,
            bitrate: OPUS_BITRATE,
            jitter_mode: "balanced".into(),
        }
    }
}

impl AudioParams {
    pub fn normalized(mut self) -> Self {
        self.sample_rate = SAMPLE_RATE;
        self.channels = 2;
        self.frame_duration_ms = FRAME_DURATION_MS;
        if ![64_000, 96_000, 128_000, 160_000, 192_000].contains(&self.bitrate) {
            self.bitrate = OPUS_BITRATE;
        }
        if !["low", "balanced", "stable", "auto"].contains(&self.jitter_mode.as_str()) {
            self.jitter_mode = "balanced".into();
        }
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub audio_port: u16,
    pub jitter_ms: u32,
    pub default_output_device: Option<usize>,
    pub device_name: String,
    pub role: String,
    pub pairing_code_mode: String,
    /// 固定配对码。**不写入 JSON 文件**（`skip_serializing`），保存到 OS keyring。
    /// 加载时若 JSON 中存在旧明文值，迁移到 keyring 后忽略文件值。
    /// P0 安全红线修复（NF-01 A4）。
    #[serde(skip_serializing, default)]
    pub fixed_pairing_code: Option<String>,
    pub jitter_mode: String,
    pub volume: f32,
    pub audio_params: AudioParams,
    pub last_receiver_addr: String,
    pub selected_capture_source: String,
    /// 关闭窗口行为："ask" | "minimize" | "quit"
    #[serde(default = "default_close_action")]
    pub close_action: String,
    /// 开机自启动（注册表项）
    #[serde(default)]
    pub auto_start: bool,
    /// 自启动后自动开启接收（仅 role=receiver 有意义）
    #[serde(default)]
    pub auto_receive_on_start: bool,
    /// 自启动后自动开启发送（仅 role=sender 有意义）
    #[serde(default)]
    pub auto_send_on_start: bool,
}

fn default_close_action() -> String {
    "ask".into()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            audio_port: DEFAULT_AUDIO_PORT,
            jitter_ms: DEFAULT_JITTER_MS,
            default_output_device: None,
            device_name: "SoundLink Receiver".into(),
            role: "receiver".into(),
            pairing_code_mode: "random".into(),
            fixed_pairing_code: None,
            jitter_mode: "balanced".into(),
            volume: 1.0,
            audio_params: AudioParams {
                jitter_mode: jitter_mode_from_ms(JITTER_BALANCED_MS).into(),
                ..AudioParams::default()
            },
            last_receiver_addr: String::new(),
            selected_capture_source: "sine".into(),
            close_action: "ask".into(),
            auto_start: false,
            auto_receive_on_start: false,
            auto_send_on_start: false,
        }
    }
}

impl AppConfig {
    pub fn load_or_default(dir: &Path) -> Self {
        let path = dir.join("app_config.json");
        let Ok(raw) = fs::read_to_string(path) else {
            // 无配置文件：尝试从 keyring 读取固定配对码。
            let mut cfg = Self::default();
            cfg.fixed_pairing_code = load_fixed_code_from_keyring();
            return cfg;
        };
        let mut cfg = match serde_json::from_str::<Self>(&raw) {
            Ok(c) => c.normalized(),
            Err(_) => {
                // JSON 解析失败：备份损坏文件并回退默认。P0 修复 NF-01 C5。
                backup_corrupt_config(dir, &raw);
                let mut def = Self::default();
                def.fixed_pairing_code = load_fixed_code_from_keyring();
                return def;
            }
        };
        // 兼容旧版明文迁移：若 JSON 中残留明文 fixed_pairing_code，迁移到 keyring 后清空。
        if let Some(plaintext) = cfg.fixed_pairing_code.take() {
            if !plaintext.is_empty() {
                save_fixed_code_to_keyring(&plaintext);
                tracing::info!("检测到旧版明文固定配对码，已迁移到 OS keyring");
            }
        }
        // 始终从 keyring 加载（以 keyring 为准）。
        cfg.fixed_pairing_code = load_fixed_code_from_keyring();
        cfg
    }

    pub fn save(&self, dir: &Path) -> Result<(), String> {
        fs::create_dir_all(dir).map_err(|e| format!("创建配置目录失败：{}", e))?;
        let path = dir.join("app_config.json");
        // 固定配对码单独写入 keyring，不进 JSON。
        if let Some(code) = &self.fixed_pairing_code {
            save_fixed_code_to_keyring(code);
        } else {
            clear_fixed_code_in_keyring();
        }
        let raw = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(path, raw).map_err(|e| format!("写入配置失败：{}", e))
    }

    pub fn normalized(mut self) -> Self {
        if !["receiver", "sender"].contains(&self.role.as_str()) {
            self.role = "receiver".into();
        }
        if !["random", "fixed"].contains(&self.pairing_code_mode.as_str()) {
            self.pairing_code_mode = "random".into();
        }
        if !["low", "balanced", "stable", "auto"].contains(&self.jitter_mode.as_str()) {
            self.jitter_mode = "balanced".into();
        }
        if !["ask", "minimize", "quit"].contains(&self.close_action.as_str()) {
            self.close_action = "ask".into();
        }
        self.volume = self.volume.clamp(0.0, 1.0);
        self.audio_params = self.audio_params.normalized();
        self
    }
}

fn jitter_mode_from_ms(ms: u32) -> &'static str {
    match ms {
        40 => "low",
        150 => "stable",
        _ => "balanced",
    }
}

/// 从 OS keyring 读取固定配对码。失败返回 None。
fn load_fixed_code_from_keyring() -> Option<String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT_FIXED_CODE).ok()?;
    let secret = entry.get_secret().ok()?;
    let code = String::from_utf8(secret.to_vec()).ok()?;
    if code.is_empty() {
        None
    } else {
        Some(code)
    }
}

/// 写入固定配对码到 OS keyring。
fn save_fixed_code_to_keyring(code: &str) {
    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT_FIXED_CODE) {
        if let Err(e) = entry.set_secret(code.as_bytes()) {
            tracing::warn!("固定配对码写入 keyring 失败：{}", e);
        }
    }
}

/// 清除 keyring 中的固定配对码（切换到随机模式时调用）。
fn clear_fixed_code_in_keyring() {
    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT_FIXED_CODE) {
        let _ = entry.delete_credential();
    }
}

/// 备份损坏的配置文件，文件名带时间戳。P0 修复 NF-01 C5。
fn backup_corrupt_config(dir: &Path, raw: &str) {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let backup_name = format!("app_config.json.corrupt-{}", timestamp);
    let backup_path = dir.join(backup_name);
    if let Err(e) = fs::write(&backup_path, raw) {
        tracing::warn!("备份损坏配置文件失败：{} -> {:?}", e, backup_path);
    } else {
        tracing::info!("已备份损坏配置文件：{:?}", backup_path);
    }
}
