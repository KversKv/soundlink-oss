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
    /// E3：是否完成首次引导。false 时启动显示 Onboarding。
    #[serde(default)]
    pub onboarding_completed: bool,
    /// F6：发送端 DRM 提示是否已展示。false 时首次开始发送弹模态。
    #[serde(default)]
    pub sender_drm_hint_seen: bool,
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
            onboarding_completed: false,
            sender_drm_hint_seen: false,
        }
    }
}

impl AppConfig {
    pub fn load_or_default(dir: &Path) -> Self {
        let path = dir.join("app_config.json");
        let Ok(raw) = fs::read_to_string(path) else {
            // 无配置文件：尝试从 keyring 读取固定配对码。
            return Self {
                fixed_pairing_code: load_fixed_code_from_keyring(),
                ..Self::default()
            };
        };
        let mut cfg = match serde_json::from_str::<Self>(&raw) {
            Ok(c) => c.normalized(),
            Err(_) => {
                // JSON 解析失败：备份损坏文件并回退默认。P0 修复 NF-01 C5。
                backup_corrupt_config(dir, &raw);
                return Self {
                    fixed_pairing_code: load_fixed_code_from_keyring(),
                    ..Self::default()
                };
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
    let entry = match keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT_FIXED_CODE) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!("keyring Entry::new 失败：{}", e);
            return None;
        }
    };
    match entry.get_secret() {
        Ok(secret) => {
            let code = String::from_utf8(secret.to_vec()).ok()?;
            if code.is_empty() {
                tracing::info!("keyring 中固定配对码为空");
                None
            } else {
                tracing::info!("keyring 固定配对码读取成功（{} 位）", code.len());
                Some(code)
            }
        }
        Err(e) => {
            tracing::warn!("keyring get_secret 失败：{}", e);
            None
        }
    }
}

/// 写入固定配对码到 OS keyring。
fn save_fixed_code_to_keyring(code: &str) {
    match keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT_FIXED_CODE) {
        Ok(entry) => match entry.set_secret(code.as_bytes()) {
            Ok(()) => tracing::info!("keyring 固定配对码写入成功（{} 位）", code.len()),
            Err(e) => tracing::warn!("固定配对码写入 keyring 失败：{}", e),
        },
        Err(e) => tracing::warn!("keyring Entry::new 失败：{}", e),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{OPUS_BITRATE, SAMPLE_RATE, FRAME_DURATION_MS};
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn default_values_are_sane() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.role, "receiver");
        assert_eq!(cfg.pairing_code_mode, "random");
        assert_eq!(cfg.jitter_mode, "balanced");
        assert_eq!(cfg.close_action, "ask");
        assert_eq!(cfg.volume, 1.0);
        assert!(!cfg.auto_start);
        assert!(!cfg.onboarding_completed);
        assert!(!cfg.sender_drm_hint_seen);
        assert_eq!(cfg.selected_capture_source, "sine");
        assert_eq!(cfg.audio_params.sample_rate, SAMPLE_RATE);
        assert_eq!(cfg.audio_params.channels, 2);
        assert_eq!(cfg.audio_params.frame_duration_ms, FRAME_DURATION_MS);
        assert_eq!(cfg.audio_params.bitrate, OPUS_BITRATE);
        assert_eq!(cfg.audio_params.jitter_mode, "balanced");
    }

    #[test]
    fn normalized_fixes_invalid_role() {
        let cfg = AppConfig {
            role: "foo".into(),
            ..AppConfig::default()
        };
        assert_eq!(cfg.normalized().role, "receiver");
    }

    #[test]
    fn normalized_fixes_invalid_pairing_mode() {
        let cfg = AppConfig {
            pairing_code_mode: "bar".into(),
            ..AppConfig::default()
        };
        assert_eq!(cfg.normalized().pairing_code_mode, "random");
    }

    #[test]
    fn normalized_fixes_invalid_jitter_mode() {
        let cfg = AppConfig {
            jitter_mode: "xyz".into(),
            ..AppConfig::default()
        };
        assert_eq!(cfg.normalized().jitter_mode, "balanced");
    }

    #[test]
    fn normalized_fixes_invalid_close_action() {
        let cfg = AppConfig {
            close_action: "abc".into(),
            ..AppConfig::default()
        };
        assert_eq!(cfg.normalized().close_action, "ask");
    }

    #[test]
    fn normalized_clamps_volume() {
        let high = AppConfig {
            volume: 1.5,
            ..AppConfig::default()
        };
        assert_eq!(high.normalized().volume, 1.0);
        let neg = AppConfig {
            volume: -0.3,
            ..AppConfig::default()
        };
        assert_eq!(neg.normalized().volume, 0.0);
    }

    #[test]
    fn normalized_fixes_invalid_bitrate() {
        let cfg = AppConfig {
            audio_params: AudioParams {
                bitrate: 123_456,
                ..AudioParams::default()
            },
            ..AppConfig::default()
        };
        assert_eq!(cfg.normalized().audio_params.bitrate, OPUS_BITRATE);
    }

    #[test]
    fn audio_params_normalized_chain() {
        let cfg = AppConfig {
            audio_params: AudioParams {
                jitter_mode: "bad".into(),
                bitrate: 99_999,
                ..AudioParams::default()
            },
            ..AppConfig::default()
        };
        let n = cfg.normalized();
        assert_eq!(n.audio_params.jitter_mode, "balanced");
        assert_eq!(n.audio_params.bitrate, OPUS_BITRATE);
    }

    #[test]
    fn load_or_default_missing_file_returns_default() {
        let dir = tempdir().unwrap();
        let cfg = AppConfig::load_or_default(dir.path());
        assert_eq!(cfg.role, "receiver");
        assert_eq!(cfg.close_action, "ask");
    }

    #[test]
    fn load_or_default_corrupt_json_backs_up_and_returns_default() {
        let dir = tempdir().unwrap();
        let cfg_path = dir.path().join("app_config.json");
        fs::write(&cfg_path, "{invalid json").unwrap();
        let cfg = AppConfig::load_or_default(dir.path());
        assert_eq!(cfg.role, "receiver");
        assert_eq!(cfg.close_action, "ask");
        // 备份文件应存在
        let mut found_backup = false;
        for entry in fs::read_dir(dir.path()).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name();
            if name.to_string_lossy().starts_with("app_config.json.corrupt-") {
                found_backup = true;
                break;
            }
        }
        assert!(found_backup, "损坏文件应生成 corrupt- 备份");
    }

    #[test]
    fn load_or_default_valid_json_loads_fields() {
        let dir = tempdir().unwrap();
        let cfg_path = dir.path().join("app_config.json");
        let raw = serde_json::json!({
            "audio_port": 47811,
            "jitter_ms": 80,
            "default_output_device": null,
            "device_name": "TestReceiver",
            "role": "sender",
            "pairing_code_mode": "fixed",
            "jitter_mode": "stable",
            "volume": 0.7,
            "audio_params": {
                "sample_rate": 48000,
                "channels": 2,
                "frame_duration_ms": 10,
                "bitrate": 96000,
                "jitter_mode": "stable"
            },
            "last_receiver_addr": "192.168.1.100:47810",
            "selected_capture_source": "sine",
            "close_action": "minimize",
            "auto_start": true,
            "auto_receive_on_start": false,
            "auto_send_on_start": true,
            "onboarding_completed": true,
            "sender_drm_hint_seen": false
        });
        fs::write(&cfg_path, raw.to_string()).unwrap();
        let cfg = AppConfig::load_or_default(dir.path());
        assert_eq!(cfg.role, "sender");
        assert_eq!(cfg.pairing_code_mode, "fixed");
        assert_eq!(cfg.jitter_mode, "stable");
        assert_eq!(cfg.close_action, "minimize");
        assert_eq!(cfg.volume, 0.7);
        assert!(cfg.auto_start);
        assert!(cfg.auto_send_on_start);
        assert!(cfg.onboarding_completed);
        assert_eq!(cfg.audio_params.bitrate, 96_000);
        assert_eq!(cfg.audio_params.jitter_mode, "stable");
    }

    #[test]
    fn save_then_load_roundtrip() {
        let dir = tempdir().unwrap();
        let mut cfg = AppConfig {
            role: "sender".into(),
            close_action: "quit".into(),
            volume: 0.5,
            device_name: "Roundtrip".into(),
            ..AppConfig::default()
        };
        cfg.audio_params.bitrate = 160_000;
        cfg.audio_params.jitter_mode = "low".into();
        cfg.save(dir.path()).unwrap();
        let loaded = AppConfig::load_or_default(dir.path());
        assert_eq!(loaded.role, "sender");
        assert_eq!(loaded.close_action, "quit");
        assert_eq!(loaded.volume, 0.5);
        assert_eq!(loaded.device_name, "Roundtrip");
        assert_eq!(loaded.audio_params.bitrate, 160_000);
        assert_eq!(loaded.audio_params.jitter_mode, "low");
        // fixed_pairing_code 为 None，JSON 中不出现该字段
        let raw = fs::read_to_string(dir.path().join("app_config.json")).unwrap();
        assert!(!raw.contains("fixed_pairing_code"));
    }

    #[test]
    fn jitter_mode_from_ms_mapping() {
        assert_eq!(jitter_mode_from_ms(40), "low");
        assert_eq!(jitter_mode_from_ms(150), "stable");
        assert_eq!(jitter_mode_from_ms(80), "balanced");
        assert_eq!(jitter_mode_from_ms(0), "balanced");
        assert_eq!(jitter_mode_from_ms(999), "balanced");
    }
}
