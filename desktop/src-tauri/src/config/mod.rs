use crate::constants::{
    DEFAULT_AUDIO_PORT, DEFAULT_JITTER_MS, FRAME_DURATION_MS, JITTER_BALANCED_MS, OPUS_BITRATE,
    SAMPLE_RATE,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

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
            return Self::default();
        };
        serde_json::from_str::<Self>(&raw)
            .map(|cfg| cfg.normalized())
            .unwrap_or_default()
    }

    pub fn save(&self, dir: &Path) -> Result<(), String> {
        fs::create_dir_all(dir).map_err(|e| format!("创建配置目录失败：{}", e))?;
        let path = dir.join("app_config.json");
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
