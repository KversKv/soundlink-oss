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
use crate::config::{AppConfig, AudioParams, Profile};
use crate::constants::{DEFAULT_AUDIO_PORT, DEFAULT_CONTROL_PORT};
use crate::device::device_identity::DeviceIdentity;
use crate::license::{self, LicenseState};
use crate::network::control_server::ControlServer;
use crate::network::discovery::{DiscoveredReceiver, MdnsBroadcaster, MdnsBrowser};
use crate::pairing::{PairingCodeManager, TrustStore, TrustedDevice};
use crate::receiver::{ReceiverEngine, ReceiverStatus};
use crate::sender::{SenderEngine, SenderStatus};
use parking_lot::Mutex;
use serde::Serialize;
use soundlink_pro_api::{
    AutomationInput, Entitlement, EntitlementHandle, ProCapabilities, StartupPlan,
};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{Manager, State};

/// 应用角色。
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[derive(Default)]
pub enum Role {
    #[default]
    Receiver,
    Sender,
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
    /// MON-01 Q4：Pro 能力对象（唯一真相源，业务代码只按能力值行事，E4）。
    pub caps: Arc<dyn ProCapabilities>,
    /// MON-01 R4：授权级别（AppState 与 Pro 实现共享同一份句柄）。
    pub entitlement: EntitlementHandle,
    /// MON-01 R4：最近一次 license 校验结论（Free 是正常状态）。
    pub license_state: Arc<parking_lot::RwLock<LicenseState>>,
    /// 本次是否由系统自启动拉起（命令行带 `--autostarted`，S5 静默启动判定输入）。
    pub autostarted: bool,
    /// 静音切换：Some(原音量) 表示当前处于静音态（PRO-5）。
    pub muted: Mutex<Option<f32>>,
    /// QR-1：分辨率快速切换服务（Pro 能力；命令层以 caps.quick_resolution_available() 门控）。
    pub qr: Arc<crate::features::quick_resolution::QrService>,
}

impl AppState {
    /// `debug`：DEBUG 模式（配对码固定 12345678）。
    /// `dump_enable`：音频各阶段 RAW Data 转储开关。
    /// `autostarted`：命令行带 `--autostarted`（本次由系统自启动拉起）。
    pub fn new(debug: bool, dump_enable: bool, autostarted: bool) -> Self {
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
        let mut config = AppConfig::load_or_default(&dir);
        let pairing = Arc::new(PairingCodeManager::with_debug(debug));
        if config.pairing_code_mode == "fixed" {
            // keyring 读取失败或码无效时，fixed_pairing_code 为 None / set_fixed_code 报错。
            // 为保持一致性：回退 config 到 random 模式，避免 issue() 走随机分支但 UI 显示 fixed。
            let ok = config.fixed_pairing_code.as_deref()
                .filter(|c| !c.is_empty())
                .map(|c| pairing.set_fixed_code(Some(c.into())).is_ok())
                .unwrap_or(false);
            if !ok {
                tracing::warn!("fixed 模式无可用长期码，回退 random；请重新设置长期配对码");
                config.pairing_code_mode = "random".into();
                config.fixed_pairing_code = None;
                // 尝试持久化回退，失败仅告警（不阻塞启动）。
                if let Err(e) = config.save(&dir) {
                    tracing::warn!("回退 random 模式持久化失败：{}", e);
                }
            }
        }
        let jitter_mode = parse_jitter_mode(&config.jitter_mode).unwrap_or(JitterMode::Balanced);
        let engine = Arc::new(ReceiverEngine::with_dump(dump_enable));
        engine.set_jitter_mode(jitter_mode);
        engine.set_volume(config.volume);
        let role = parse_role(&config.role).unwrap_or_default();
        // MON-01 R4：启动时加载并验签 license 一次（全程离线）。
        // Free 是正常状态：加载失败仅 info 级日志，不 warn 不 error。
        let device_id = identity.device_id.clone();
        let license_state = license::load_and_validate(&dir, &device_id);
        let entitlement: EntitlementHandle = Arc::new(parking_lot::RwLock::new(
            if license_state.is_active() {
                Entitlement::Pro
            } else {
                Entitlement::Free
            },
        ));
        // MON-01 Q4：能力对象来自 soundlink-pro crate（免费实现 / 私有实现）。
        // 门控判定一次性完成，不进音频热路径（E5）。
        let caps = soundlink_pro::capabilities(entitlement.clone());
        Self {
            engine,
            sender: Arc::new(
                SenderEngine::with_trust(trust.clone(), dump_enable).with_caps(caps.clone()),
            ),
            pairing,
            identity: Arc::new(Mutex::new(identity)),
            trust,
            selected_device: Arc::new(Mutex::new(config.default_output_device)),
            control: Mutex::new(None),
            mdns: Mutex::new(None),
            device_name: Mutex::new(config.device_name.clone()),
            role: Mutex::new(role),
            config: Arc::new(Mutex::new(config)),
            config_dir: dir.clone(),
            dump_enable,
            identity_load_failed,
            caps,
            entitlement,
            license_state: Arc::new(parking_lot::RwLock::new(license_state)),
            autostarted,
            muted: Mutex::new(None),
            qr: crate::features::quick_resolution::QrService::new(dir.clone()),
        }
    }

    /// MON-01 S3：构造启动自动化判定输入。
    ///
    /// `last_peer_device_id` 仅当该设备仍在信任存储且带连接信息时提供
    /// （否则自动连接必然失败，不应出现在计划里）。
    pub fn automation_input(&self) -> AutomationInput {
        let cfg = self.config.lock();
        let role = *self.role.lock();
        let last_peer_device_id = cfg.last_peer_device_id.clone().filter(|id| {
            self.trust
                .lock()
                .get(id)
                .map(|d| d.host.is_some() && d.control_port.is_some())
                .unwrap_or(false)
        });
        AutomationInput {
            auto_start: cfg.auto_start,
            auto_receive_on_start: cfg.auto_receive_on_start,
            auto_send_on_start: cfg.auto_send_on_start,
            role: match role {
                Role::Receiver => soundlink_pro_api::Role::Receiver,
                Role::Sender => soundlink_pro_api::Role::Sender,
            },
            launched_via_autostart: self.autostarted,
            last_peer_device_id,
        }
    }

    /// MON-01 S3/S5：启动计划（免费实现恒 None；门控只在此一处判定）。
    pub fn startup_plan(&self) -> Option<StartupPlan> {
        self.caps.startup_plan(&self.automation_input())
    }

    /// MON-01 R5：刷新授权状态（激活/反激活后调用）。
    /// 写 license_state 与 entitlement 句柄（Pro 实现即刻感知，无需重启）。
    pub fn set_license_state(&self, state: LicenseState) {
        let active = state.is_active();
        *self.license_state.write() = state;
        *self.entitlement.write() = if active {
            Entitlement::Pro
        } else {
            Entitlement::Free
        };
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
            Some(app.clone()),
            // MON-01 S1：设备记忆上限来自能力对象。
            Some(s.caps.clone()),
        );
        let bind = format!("0.0.0.0:{}", DEFAULT_CONTROL_PORT);
        control.start(&bind).await?;
        *s.control.lock() = Some(control);
    }
    // MON-01 S15：托盘菜单文字随状态翻转（「开始接收」↔「停止接收」）。
    tray::refresh_tray(&app);

    Ok(StartResult {
        pairing_code: code,
        control_port: DEFAULT_CONTROL_PORT,
        audio_port: DEFAULT_AUDIO_PORT,
        device_id,
    })
}

/// 停止接收器：停止控制服务器、mDNS 广播、UDP 接收。
#[tauri::command]
pub fn stop_receiver(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let s = state.inner();
    if let Some(c) = s.control.lock().take() {
        c.stop();
    }
    if let Some(m) = s.mdns.lock().take() {
        m.stop();
    }
    s.engine.stop();
    // MON-01 S15：托盘菜单文字随状态翻转。
    tray::refresh_tray(&app);
    Ok(())
}

/// 优雅退出清理（D3）：停止 sender（带 1s 超时）+ receiver + control + mDNS。
/// 在 quit_app 与 tray quit 路径调用，避免依赖 Drop 导致退出卡顿或端口残留。
pub async fn cleanup_before_quit(state: &AppState) {
    // QR-1：restore_on_app_exit 开启时先恢复原始分辨率（在停止引擎前做，失败不阻断退出）。
    state.qr.restore_session_originals();
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

/// 本机局域网 IP 地址（配对码卡片显示本机地址用）。
/// `local_ip()` 返回系统默认路由出口的 IPv4，已天然排除回环与链路本地。
#[derive(Debug, Serialize)]
pub struct LocalAddressInfo {
    pub ip: String,
    pub control_port: u16,
    pub audio_port: u16,
}

#[tauri::command]
pub fn get_local_addresses() -> Result<Vec<LocalAddressInfo>, String> {
    use local_ip_address::local_ip;
    // 单个候选 IP（系统默认路由出口）。多网卡场景下足够覆盖大多数用户；
    // 若需枚举全部接口，可后续扩展为 list_afinet_netifas。
    match local_ip() {
        Ok(ip) => Ok(vec![LocalAddressInfo {
            ip: ip.to_string(),
            control_port: DEFAULT_CONTROL_PORT,
            audio_port: DEFAULT_AUDIO_PORT,
        }]),
        Err(e) => Err(format!("无法获取本机 IP：{}", e)),
    }
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
        repository: "https://github.com/KversKv/soundlink-oss",
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
    // 双重保险：以 PairingCodeManager.fixed_code() 实际值为准决定 mode，
    // 避免 config.pairing_code_mode 与 keyring 实际状态不一致导致 UI 显示错位。
    let fixed = s.pairing.fixed_code();
    let mode = match &fixed {
        Some(_) => "fixed".to_string(),
        None => "random".to_string(),
    };
    Ok(DesktopSettings {
        device_name: s.device_name.lock().clone(),
        role: role_as_str(*s.role.lock()).into(),
        selected_device: *s.selected_device.lock(),
        jitter_mode: s.engine.jitter_mode().as_str().into(),
        volume: s.engine.volume(),
        pairing: PairingSettings {
            mode,
            fixed_code: fixed.unwrap_or_default(),
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
            // set_fixed_code 会清空 current，这里重新 issue 让长期码立即可用。
            // 不再依赖 current().is_some() 判断（已被清空，恒为 false）。
            s.pairing.issue();
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
    // N1/N2：手动模式下把目标码率热下发到发送端；auto 模式开启自适应（建议值自动生效）。
    let adaptive = params.jitter_mode == "auto";
    s.sender.set_bitrate_adaptive(adaptive);
    if !adaptive {
        s.sender.set_target_bitrate(params.bitrate);
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
    let params = s.config.lock().audio_params.clone().normalized();
    let loss_rate = receiver.loss_rate;
    let jitter_ms = receiver.jitter_ms;
    let recommended = if receiver.recommended_bitrate > 0 {
        receiver.recommended_bitrate
    } else {
        sender.recommended_bitrate
    };
    // O1：样本不足（未开流或收包过少）时保持当前参数，不给乐观推荐（假阳性）。
    // 与 receiver recommend_bitrate 的 PROBE_MIN_PACKETS 判据对齐。
    if receiver.packets_recv < crate::constants::PROBE_MIN_PACKETS && recommended == 0 {
        tracing::info!(
            "自动探测：样本不足（packets_recv={}），保持当前参数 {}kbps/{}",
            receiver.packets_recv,
            params.bitrate / 1000,
            params.jitter_mode
        );
        return Ok(params);
    }
    let mut params = params;
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
        .await?;
    // MON-01 S7：连接成功后记录上次对端设备（供 Pro 跨启动自动重连）。
    record_last_peer(s, &s.sender.status().receiver_device_id);
    // MON-01 S15：托盘菜单文字随状态翻转。
    tray::refresh_tray(&app);
    Ok(())
}

/// 停止发送端。
#[tauri::command]
pub async fn stop_sender(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    state.inner().sender.stop().await;
    // MON-01 S15：托盘菜单文字随状态翻转。
    tray::refresh_tray(&app);
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
        .await?;
    // MON-01 S7：连接成功后记录上次对端设备。
    record_last_peer(s, &device_id);
    Ok(())
}

/// MON-01 S7：写入上次对端 device_id（空串忽略）。免费版也写入，只是不消费。
fn record_last_peer(s: &AppState, peer_device_id: &str) {
    if peer_device_id.is_empty() {
        return;
    }
    let changed = {
        let mut cfg = s.config.lock();
        if cfg.last_peer_device_id.as_deref() == Some(peer_device_id) {
            false
        } else {
            cfg.last_peer_device_id = Some(peer_device_id.to_string());
            true
        }
    };
    if changed {
        if let Err(e) = save_config(s) {
            tracing::warn!("记录上次对端设备失败：{}", e);
        }
    }
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
    /// 「开机自启动」是否可配置。**免费可用**（恒 true）。
    pub autostart_available: bool,
    /// 「启动后自动开启接收/发送」是否可配置。false 时前端置灰（Pro 能力）。
    pub automation_available: bool,
    /// MON-01 S10：配置档是否可用（Pro 能力）。
    pub profiles_available: bool,
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
        autostart_available: state.caps.autostart_available(),
        automation_available: state.caps.automation_available(),
        profiles_available: state.caps.profiles().is_some(),
    })
}

/// 批量保存设置：写入 config + 同步 autostart 注册表项（仅当 `auto_start` 被显式设置）。
// Tauri command 的参数需与前端字段一一对应，无法折叠成结构体。
#[allow(clippy::too_many_arguments)]
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
    // 「开机自启动」免费：直接写入并同步注册项。
    // 「启动后自动收/发」为 Pro 能力（automation_available）：不可用时忽略并返回当前值
    // （不报错、不写入）。
    let automation_available = state.caps.automation_available();
    if !automation_available && (auto_receive_on_start.is_some() || auto_send_on_start.is_some()) {
        tracing::info!("自动收/发为 Pro 能力，本次写入被忽略（保持当前值）");
    }
    {
        let mut cfg = state.config.lock();
        if let Some(v) = close_action {
            if !["ask", "minimize", "quit"].contains(&v.as_str()) {
                return Err(format!("非法 close_action：{}", v));
            }
            cfg.close_action = v;
        }
        // 开机自启：免费，始终写入。
        if let Some(v) = auto_start {
            cfg.auto_start = v;
        }
        if automation_available {
            if let Some(v) = auto_receive_on_start {
                cfg.auto_receive_on_start = v;
            }
            if let Some(v) = auto_send_on_start {
                cfg.auto_send_on_start = v;
            }
        }
        if let Some(v) = onboarding_completed {
            cfg.onboarding_completed = v;
        }
        if let Some(v) = sender_drm_hint_seen {
            cfg.sender_drm_hint_seen = v;
        }
    }
    save_config(state.inner())?;
    // 开机自启免费：始终同步注册项。
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
    // 开机自启为免费功能（autostart_available 恒 true），不做授权门控。
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

// ──────────────────────────── MON-01 R5：Pro 授权 ────────────────────────────

/// 授权状态（设置页「授权」区块）。
#[derive(Debug, Clone, Serialize)]
pub struct LicenseInfo {
    /// "free" | "pro"
    pub entitlement: String,
    /// "free" | "active" | "invalid" | "expired" | "revoked" | "device_mismatch"
    pub state: String,
    /// Invalid 的详细原因（其余状态为 None）。
    pub detail: Option<String>,
    /// 买家标识掩码回显（前 4 后 2，避免截图泄露完整指纹/订单号）。
    pub sub_masked: Option<String>,
    /// 本机设备指纹（单向哈希 10 位短码；下单时提供给卖家）。
    pub fingerprint: String,
    /// 本构建是否含 Pro 逻辑（社区构建为 false，前端据此显示「本构建不含 Pro」）。
    pub pro_build: bool,
}

fn mask_sub(sub: &str) -> String {
    let chars: Vec<char> = sub.chars().collect();
    if chars.len() > 6 {
        let head: String = chars[..4].iter().collect();
        let tail: String = chars[chars.len() - 2..].iter().collect();
        format!("{}…{}", head, tail)
    } else {
        "…".into()
    }
}

fn license_info(state: &AppState) -> LicenseInfo {
    let ls = state.license_state.read().clone();
    let device_id = state.identity.lock().device_id.clone();
    let fingerprint = license::fingerprint::fingerprint_candidates(&device_id)
        .first()
        .cloned()
        .unwrap_or_default();
    LicenseInfo {
        entitlement: match *state.entitlement.read() {
            Entitlement::Pro => "pro".into(),
            Entitlement::Free => "free".into(),
        },
        state: ls.state_str().into(),
        detail: ls.detail().map(String::from),
        sub_masked: ls.active_sub().map(mask_sub),
        fingerprint,
        // 构建形态标识仅用于 UI 文案（不做门控，G6）。
        pro_build: soundlink_pro::EDITION == "official",
    }
}

/// 查询授权状态。
#[tauri::command]
pub fn get_license_status(state: State<'_, AppState>) -> Result<LicenseInfo, String> {
    Ok(license_info(state.inner()))
}

/// 激活 license：验签通过则写 keyring + 更新 entitlement + emit `license-changed`。
/// 任何非 Active 一律保持免费版（E1），并返回带原因的 LicenseInfo（不抛错，UI 内联展示）。
#[tauri::command]
pub fn activate_license(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    key: String,
) -> Result<LicenseInfo, String> {
    use tauri::Emitter;
    let s = state.inner();
    let device_id = s.identity.lock().device_id.clone();
    let validated = license::validate(&key, &device_id);
    if validated.is_active() {
        license::save_license_text(&s.config_dir, key.trim())?;
        s.set_license_state(validated);
        let info = license_info(s);
        let _ = app.emit("license-changed", &info);
        Ok(info)
    } else {
        // 不写入、不改变当前授权状态；仅返回原因供 UI 展示。
        let mut info = license_info(s);
        info.state = validated.state_str().into();
        info.detail = validated.detail().map(String::from);
        Ok(info)
    }
}

/// 反激活：清除 keyring 与文件，回落 Free（给用户「换机前先释放」的确定性）。
#[tauri::command]
pub fn deactivate_license(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<LicenseInfo, String> {
    use tauri::Emitter;
    let s = state.inner();
    license::clear_license(&s.config_dir);
    s.set_license_state(LicenseState::Free);
    let info = license_info(s);
    let _ = app.emit("license-changed", &info);
    Ok(info)
}

// ──────────────────────── MON-01 S3：启动计划下沉到 Rust ────────────────────────

/// 启动时应自动执行的计划（免费实现恒 None）。
/// 前端只负责「拿到 plan 就执行对应现有命令 + 更新 UI 状态」。
#[tauri::command]
pub fn resolve_startup_plan(state: State<'_, AppState>) -> Result<Option<StartupPlan>, String> {
    Ok(state.startup_plan())
}

// ──────────────────────────── MON-01 S11：配置档 ────────────────────────────

/// 配置档列表 + 可用性。免费下 `available=false`、列表为空（字段仍保留于配置，E6）。
#[derive(Debug, Clone, Serialize)]
pub struct ProfilesInfo {
    pub available: bool,
    pub max: usize,
    pub active_id: Option<String>,
    pub profiles: Vec<Profile>,
}

fn profile_gate(s: &AppState) -> Result<usize, String> {
    s.caps
        .profiles()
        .map(|p| p.max_profiles())
        .ok_or_else(|| "多套配置为 Pro 功能，当前构建/授权下不可用".to_string())
}

#[tauri::command]
pub fn list_profiles(state: State<'_, AppState>) -> Result<ProfilesInfo, String> {
    let s = state.inner();
    let cfg = s.config.lock();
    match s.caps.profiles() {
        Some(store) => Ok(ProfilesInfo {
            available: true,
            max: store.max_profiles(),
            active_id: cfg.active_profile.clone(),
            profiles: cfg.profiles.clone(),
        }),
        None => Ok(ProfilesInfo {
            available: false,
            max: 0,
            active_id: None,
            profiles: Vec::new(),
        }),
    }
}

/// 把当前运行配置快照为一个新档。
#[tauri::command]
pub fn save_profile(state: State<'_, AppState>, name: String) -> Result<Profile, String> {
    let s = state.inner();
    let max = profile_gate(s)?;
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("配置档名称不能为空".into());
    }
    let profile = {
        let cfg = s.config.lock();
        if cfg.profiles.len() >= max {
            return Err(format!("配置档已达上限（{} 个）", max));
        }
        if cfg.profiles.iter().any(|p| p.name == name) {
            return Err(format!("已存在同名配置档：{}", name));
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Profile {
            id: format!("prof-{}", now),
            name,
            output_device: *s.selected_device.lock(),
            jitter_mode: s.engine.jitter_mode().as_str().into(),
            volume: s.engine.volume(),
            audio_params: cfg.audio_params.clone(),
            role: role_as_str(*s.role.lock()).into(),
            peer_device_id: cfg.last_peer_device_id.clone(),
        }
    };
    {
        let mut cfg = s.config.lock();
        cfg.profiles.push(profile.clone());
        cfg.active_profile = Some(profile.id.clone());
    }
    save_config(s)?;
    Ok(profile)
}

/// 应用配置档结果。`restart_required=true` 提示用户需重启流才完全生效（不静默重启）。
#[derive(Debug, Clone, Serialize)]
pub struct ApplyProfileResult {
    pub profile: Profile,
    pub restart_required: bool,
}

#[tauri::command]
pub fn apply_profile(
    state: State<'_, AppState>,
    id: String,
) -> Result<ApplyProfileResult, String> {
    let s = state.inner();
    profile_gate(s)?;
    let profile = {
        let cfg = s.config.lock();
        cfg.profiles
            .iter()
            .find(|p| p.id == id)
            .cloned()
            .ok_or_else(|| format!("配置档不存在：{}", id))?
    };
    // 复用既有命令内部逻辑（不重复实现）。
    let role = parse_role(&profile.role).unwrap_or_default();
    *s.role.lock() = role;
    *s.selected_device.lock() = profile.output_device;
    let jitter = parse_jitter_mode(&profile.jitter_mode).unwrap_or(JitterMode::Balanced);
    s.engine.set_jitter_mode(jitter);
    s.engine.set_volume(profile.volume.clamp(0.0, 1.0));
    let params = profile.audio_params.clone().normalized();
    let restart_required = params.restart_required();
    let adaptive = params.jitter_mode == "auto";
    s.sender.set_bitrate_adaptive(adaptive);
    if !adaptive {
        s.sender.set_target_bitrate(params.bitrate);
    }
    {
        let mut cfg = s.config.lock();
        cfg.role = role_as_str(role).into();
        cfg.default_output_device = profile.output_device;
        cfg.jitter_mode = jitter.as_str().into();
        cfg.volume = s.engine.volume();
        cfg.audio_params = params;
        cfg.active_profile = Some(profile.id.clone());
    }
    save_config(s)?;
    Ok(ApplyProfileResult {
        profile,
        restart_required,
    })
}

#[tauri::command]
pub fn delete_profile(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let s = state.inner();
    profile_gate(s)?;
    {
        let mut cfg = s.config.lock();
        let before = cfg.profiles.len();
        cfg.profiles.retain(|p| p.id != id);
        if cfg.profiles.len() == before {
            return Err(format!("配置档不存在：{}", id));
        }
        if cfg.active_profile.as_deref() == Some(id.as_str()) {
            cfg.active_profile = None;
        }
    }
    save_config(s)
}

#[tauri::command]
pub fn rename_profile(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> Result<(), String> {
    let s = state.inner();
    profile_gate(s)?;
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("配置档名称不能为空".into());
    }
    {
        let mut cfg = s.config.lock();
        if cfg.profiles.iter().any(|p| p.name == name && p.id != id) {
            return Err(format!("已存在同名配置档：{}", name));
        }
        let profile = cfg
            .profiles
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| format!("配置档不存在：{}", id))?;
        profile.name = name;
    }
    save_config(s)
}

// ──────────────────── MON-01 S13/S14/S15：快捷键与托盘状态 ────────────────────

/// 当前生效的全局快捷键（能力驱动；免费仅「显示主窗口」）。
#[tauri::command]
pub fn get_shortcuts(
    state: State<'_, AppState>,
) -> Result<Vec<soundlink_pro_api::ShortcutBinding>, String> {
    let custom = state.config.lock().shortcuts.clone();
    Ok(state.caps.shortcuts(&custom))
}

/// 托盘直控状态（托盘菜单与前端 Pro 区块共用）。
#[derive(Debug, Clone, Serialize)]
pub struct TrayStateInfo {
    pub receiver_running: bool,
    pub sender_running: bool,
    pub muted: bool,
    pub tray_items: Vec<String>,
    pub profiles: Vec<Profile>,
    pub active_profile: Option<String>,
}

pub fn tray_state_info(s: &AppState) -> TrayStateInfo {
    let cfg = s.config.lock();
    TrayStateInfo {
        receiver_running: s.engine.is_running(),
        sender_running: s.sender.is_running(),
        muted: s.muted.lock().is_some(),
        tray_items: s
            .caps
            .tray_items()
            .iter()
            .map(|i| format!("{:?}", i))
            .collect(),
        profiles: if s.caps.profiles().is_some() {
            cfg.profiles.clone()
        } else {
            Vec::new()
        },
        active_profile: cfg.active_profile.clone(),
    }
}

#[tauri::command]
pub fn get_tray_state(state: State<'_, AppState>) -> Result<TrayStateInfo, String> {
    Ok(tray_state_info(state.inner()))
}

/// 静音切换（接收输出）。返回静音后的状态：true=已静音。
/// 静音时记住原音量并置 0；取消静音恢复原音量。
#[tauri::command]
pub fn toggle_mute(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(toggle_mute_inner(state.inner()))
}

pub fn toggle_mute_inner(s: &AppState) -> bool {
    let mut muted = s.muted.lock();
    match *muted {
        Some(prev) => {
            s.engine.set_volume(prev);
            *muted = None;
            false
        }
        None => {
            let prev = s.engine.volume();
            s.engine.set_volume(0.0);
            *muted = Some(prev);
            true
        }
    }
}

// ──────────────────────── MON-01 S2：设备记忆配额（UI 标注用） ────────────────────────

/// 设备记忆配额：两个方向各自 `used/max`。
#[derive(Debug, Clone, Serialize)]
pub struct TrustQuota {
    pub max: usize,
    /// 接收端视角：信任的发送端数量。
    pub senders_used: usize,
    /// 发送端视角：信任的接收端数量。
    pub receivers_used: usize,
}

#[tauri::command]
pub fn get_trust_quota(state: State<'_, AppState>) -> Result<TrustQuota, String> {
    let s = state.inner();
    let trust = s.trust.lock();
    Ok(TrustQuota {
        max: s.caps.max_remembered_devices(),
        senders_used: trust.list().iter().filter(|d| d.host.is_none()).count(),
        receivers_used: trust.list().iter().filter(|d| d.host.is_some()).count(),
    })
}

// ──────────────────── MON-01 S9/S15：自动发送目标与托盘开关 ────────────────────

/// MON-01 S9：自动发送目标选择 —— `last_peer_device_id` 优先，回退 `last_seen` 最新。
/// 只考虑带完整连接信息（host + control_port）的信任接收端。
fn pick_auto_send_target(cfg: &AppConfig, trust: &TrustStore) -> Option<String> {
    let usable = |d: &TrustedDevice| d.host.is_some() && d.control_port.is_some();
    if let Some(last) = &cfg.last_peer_device_id {
        if let Some(d) = trust.get(last).filter(|d| usable(d)) {
            return Some(d.device_id.clone());
        }
    }
    trust
        .list()
        .iter()
        .filter(|d| usable(d))
        .max_by_key(|d| d.last_seen)
        .map(|d| d.device_id.clone())
}

/// 解析自动发送目标（前端自动启动与托盘直控共用）。
#[tauri::command]
pub fn resolve_auto_send_target(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let s = state.inner();
    let cfg = s.config.lock();
    let trust = s.trust.lock();
    Ok(pick_auto_send_target(&cfg, &trust))
}

/// MON-01 S15：托盘「开始/停止接收」共用逻辑。返回 "started" / "stopped"。
pub async fn toggle_receiver_inner(app: tauri::AppHandle) -> Result<String, String> {
    let state: State<'_, AppState> = app.state();
    if state.engine.is_running() {
        stop_receiver(app.clone(), state)?;
        Ok("stopped".into())
    } else {
        start_receiver(app.clone(), state).await?;
        Ok("started".into())
    }
}

/// MON-01 S15：托盘「开始/停止发送」共用逻辑（目标按 S9 规则选择）。
pub async fn toggle_sender_inner(app: tauri::AppHandle) -> Result<String, String> {
    let state: State<'_, AppState> = app.state();
    if state.sender.is_running() {
        stop_sender(app.clone(), state).await?;
        return Ok("stopped".into());
    }
    let target = {
        let cfg = state.config.lock();
        let trust = state.trust.lock();
        pick_auto_send_target(&cfg, &trust)
    };
    match target {
        Some(device_id) => {
            connect_trusted_receiver(state, device_id, None).await?;
            Ok("started".into())
        }
        None => Err("没有可自动连接的信任接收端".into()),
    }
}

#[cfg(test)]
mod pick_target_tests {
    use super::*;

    fn trusted(id: &str, last_seen: u64, with_addr: bool) -> TrustedDevice {
        TrustedDevice {
            device_id: id.into(),
            identity_pub_b64: "pub".into(),
            name: None,
            last_seen,
            host: with_addr.then(|| "192.168.1.2".to_string()),
            control_port: with_addr.then_some(47820),
            audio_port: with_addr.then_some(47821),
        }
    }

    #[test]
    fn prefers_last_peer_when_usable() {
        let mut trust = TrustStore::in_memory();
        trust.add(trusted("dev-old", 1000, true), 8).unwrap();
        trust.add(trusted("dev-new", 2000, true), 8).unwrap();
        let cfg = AppConfig {
            last_peer_device_id: Some("dev-old".into()),
            ..AppConfig::default()
        };
        // S9：last_peer 优先于 last_seen 最新。
        assert_eq!(pick_auto_send_target(&cfg, &trust), Some("dev-old".into()));
    }

    #[test]
    fn falls_back_to_latest_last_seen() {
        let mut trust = TrustStore::in_memory();
        trust.add(trusted("dev-old", 1000, true), 8).unwrap();
        trust.add(trusted("dev-new", 2000, true), 8).unwrap();
        let cfg = AppConfig::default();
        assert_eq!(pick_auto_send_target(&cfg, &trust), Some("dev-new".into()));
    }

    #[test]
    fn skips_entries_without_addr() {
        let mut trust = TrustStore::in_memory();
        // 接收端视角的信任条目（无 host）不参与自动发送目标选择。
        trust.add(trusted("phone-a", 3000, false), 8).unwrap();
        trust.add(trusted("dev-new", 2000, true), 8).unwrap();
        let cfg = AppConfig::default();
        assert_eq!(pick_auto_send_target(&cfg, &trust), Some("dev-new".into()));
    }

    #[test]
    fn none_when_no_usable_receiver() {
        let trust = TrustStore::in_memory();
        let cfg = AppConfig::default();
        assert_eq!(pick_auto_send_target(&cfg, &trust), None);
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
