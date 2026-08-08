//! QR-1 数据模型（display.md §九）+ 统一错误类型（§十五）。
//!
//! 序列化全部 camelCase，与前端 `features/quickResolution/types.ts` 一一对应。

use qr_ipc::{ActivationMethod, MonitorKey, RegVariant};
use serde::{Deserialize, Serialize};

/// 模式生命周期状态机（display.md §二）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModeState {
    Draft,
    Validated,
    Provisioning,
    Ready,
    Active,
    Stale,
    Failed,
}

impl ModeState {
    /// 可被 Apply 快切（已在系统模式列表中）。
    pub fn is_ready(self) -> bool {
        matches!(self, ModeState::Ready | ModeState::Active)
    }
    /// 待预置（攒批注入）。
    pub fn is_pending(self) -> bool {
        matches!(self, ModeState::Draft | ModeState::Validated | ModeState::Failed)
    }
}

/// 模式进入系统列表的路径。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProvisionPath {
    /// 系统原生/从系统导入。
    System,
    /// NVAPI 自定义分辨率。
    Nvapi,
    /// EDID Override 注入。
    Edid,
}

/// 目标显示器绑定（display.md §8.1 三层标识）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ModeTarget {
    Primary,
    Index { index: u32 },
    Key { key: MonitorKey },
}

impl Default for ModeTarget {
    fn default() -> Self {
        ModeTarget::Primary
    }
}

/// 色彩格式（前端编辑器高级区）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorFormat {
    RGB,
    YCbCr444,
    YCbCr422,
    YCbCr420,
}

impl ColorFormat {
    pub fn to_bandwidth(self) -> qr_bandwidth::ColorFormat {
        match self {
            ColorFormat::RGB => qr_bandwidth::ColorFormat::Rgb,
            ColorFormat::YCbCr444 => qr_bandwidth::ColorFormat::YCbCr444,
            ColorFormat::YCbCr422 => qr_bandwidth::ColorFormat::YCbCr422,
            ColorFormat::YCbCr420 => qr_bandwidth::ColorFormat::YCbCr420,
        }
    }
}

/// GPU 缩放策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScalingMode {
    Aspect,
    Fullscreen,
    Centered,
    NoScaling,
}

/// Timing 标准（与 qr-edid 对齐；`manual` 参数独立字段承载）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimingStandardKind {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "cvt-rb2")]
    CvtRb2,
    #[serde(rename = "cvt-rb3")]
    CvtRb3,
    #[serde(rename = "manual")]
    Manual,
}

/// 手动 timing 参数（高级用户）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualTiming {
    pub h_front: u32,
    pub h_sync: u32,
    pub h_back: u32,
    pub v_front: u32,
    pub v_sync: u32,
    pub v_back: u32,
    pub h_sync_pol: bool,
    pub v_sync_pol: bool,
}

/// 结构化错误记录（模式行内展示）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeError {
    pub code: String,
    pub message: String,
    pub at: i64,
}

/// 一条分辨率模式。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayModeEntry {
    pub id: String,
    pub label: String,
    pub width: u32,
    pub height: u32,
    /// 整数 Hz（需求边界：不支持小数刷新率）。
    pub refresh_hz: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bit_depth: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_format: Option<ColorFormat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scaling: Option<ScalingMode>,
    #[serde(default)]
    pub target: ModeTarget,
    #[serde(default)]
    pub timing_standard: TimingStandardKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual_timing: Option<ManualTiming>,
    #[serde(default)]
    pub state: ModeState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provision_path: Option<ProvisionPath>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<ModeError>,
    #[serde(default)]
    pub pinned_to_tray: bool,
    #[serde(default)]
    pub order: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hotkey: Option<String>,
    /// 切换此模式时跳过 15 秒确认窗（高危，display.md §7.4 反向开关，默认 false）。
    #[serde(default)]
    pub skip_confirm: bool,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<i64>,
}

impl DisplayModeEntry {
    /// 简要描述（托盘/确认窗用）：`1920×1440 @480Hz`。
    pub fn brief(&self) -> String {
        format!("{}×{} @{}Hz", self.width, self.height, self.refresh_hz)
    }
}

#[allow(clippy::derivable_impls)] // 显式实现，避免 derive 带来的语义隐式性
impl Default for TimingStandardKind {
    fn default() -> Self {
        TimingStandardKind::Auto
    }
}

#[allow(clippy::derivable_impls)]
impl Default for ModeState {
    fn default() -> Self {
        ModeState::Draft
    }
}

/// DSC 判定覆盖（display.md §6.2 手动覆盖）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DscOverride {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "force-on")]
    ForceOn,
    #[serde(rename = "force-off")]
    ForceOff,
}

/// 功能设置 + 模式列表（`quick_resolution.json`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickResolutionSettings {
    pub schema_version: u32,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub show_in_tray: bool,
    #[serde(default = "default_max_tray_items")]
    pub max_tray_items: u32,
    #[serde(default = "default_true")]
    pub confirm_before_apply: bool,
    #[serde(default = "default_auto_revert_seconds")]
    pub auto_revert_seconds: u32,
    #[serde(default)]
    pub restore_on_app_exit: bool,
    #[serde(default)]
    pub dsc_override: DscOverride,
    /// EDID 注入总开关（默认 false；首次开启需风险二次确认）。
    #[serde(default)]
    pub allow_edid_override: bool,
    #[serde(default)]
    pub enable_global_hotkeys: bool,
    /// helper 是否已完成一次性安装（计划任务注册成功）。
    #[serde(default)]
    pub helper_installed: bool,
    #[serde(default)]
    pub modes: Vec<DisplayModeEntry>,
}

fn default_true() -> bool {
    true
}
fn default_max_tray_items() -> u32 {
    8
}
fn default_auto_revert_seconds() -> u32 {
    15
}

#[allow(clippy::derivable_impls)]
impl Default for DscOverride {
    fn default() -> Self {
        DscOverride::Auto
    }
}

impl Default for QuickResolutionSettings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            enabled: false,
            show_in_tray: true,
            max_tray_items: 8,
            confirm_before_apply: true,
            auto_revert_seconds: 15,
            restore_on_app_exit: false,
            dsc_override: DscOverride::Auto,
            allow_edid_override: false,
            enable_global_hotkeys: false,
            helper_installed: false,
            modes: Vec::new(),
        }
    }
}

/// 当前生效模式简报（托盘「当前:」行）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentModeBrief {
    pub display_index: u32,
    pub text: String,
    pub width: u32,
    pub height: u32,
    pub refresh_hz: u32,
}

/// 系统已注册模式（GDI 枚举结果）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemMode {
    pub width: u32,
    pub height: u32,
    pub refresh_hz: u32,
    pub bits_per_pel: u32,
}

impl SystemMode {
    pub fn matches(&self, m: &DisplayModeEntry) -> bool {
        self.width == m.width && self.height == m.height && self.refresh_hz == m.refresh_hz
    }
}

/// 显示器信息（三层标识体系，§8.1）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayInfo {
    /// UI 编号 1..N（CCD source id 排序）。
    pub index: u32,
    /// 稳定主键。
    pub key: MonitorKey,
    /// GDI 名（运行时解析，不持久化）。
    pub gdi_name: String,
    pub friendly_name: String,
    pub is_primary: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current: Option<SystemMode>,
    /// 链路信息（M3 填充；M1 恒 None）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<DisplayLinkInfo>,
    /// DSC 判定（M3 填充；M1 恒 Unknown）。
    pub dsc: DscState,
    /// EDID 上报的最大像素时钟（kHz，来自 range limits）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_pixel_clock_khz: Option<u32>,
}

/// 链路信息（DSC 徽标/诊断）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayLinkInfo {
    pub lane_count: u8,
    pub rate_per_lane_gbps: f32,
    /// 规范化链路标签（如 "DP2.1 UHBR13.5 ×4"）。
    pub link_label: String,
    pub bpc: Option<u8>,
    pub color_format: Option<String>,
    pub available_gbps: f32,
    /// 链路信息来源："nvapi" | "inferred" | "unknown"。
    pub source: String,
}

/// DSC 状态（display.md §6.2）。
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum DscState {
    Active,
    Inactive,
    LikelyActive { confidence: f32, basis: Vec<String> },
    Unknown { reason: String, #[serde(skip_serializing_if = "Vec::is_empty", default)] debug: Vec<String> },
    ForcedByUser { on: bool },
}

#[allow(clippy::derivable_impls)]
impl Default for DscState {
    fn default() -> Self {
        DscState::Unknown { reason: "未检测".into(), debug: Vec::new() }
    }
}

/// 能力档案（display.md §5.1，缓存键 = GPU+驱动+EDID+连接器）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityProfile {
    pub key: String,
    /// NVAPI 自定义分辨率可用性（三态：DSC 开启后通常 Blocked）。
    pub nvapi_custom: TriState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nvapi_custom_last_status: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edid_reg_variant: Option<RegVariant>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activation: Option<ActivationMethod>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_extension_blocks: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub free_dtd_slots: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub displayid_supported: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_max_pixel_clock_khz: Option<u32>,
    pub probed_at: i64,
    #[serde(default)]
    pub probe_log_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriState {
    Available,
    Blocked,
    Unknown,
}

/// 模式校验报告（`qr_validate_mode`）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationReport {
    pub ok: bool,
    pub errors: Vec<String>,
    /// 是否已在系统模式列表（false = 保存后需预置）。
    pub in_system_list: bool,
    pub pixel_clock_khz: u64,
    /// EDID 上报上限校验（None = 无上限信息）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exceeds_monitor_limit: Option<bool>,
    /// 带宽可行性（M3 有链路信息后填充）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feasibility: Option<qr_bandwidth::Feasibility>,
}

/// 批量预置报告。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvisionReport {
    pub succeeded: Vec<String>,
    pub failed: Vec<String>,
    pub activation: String,
    pub backup_id: String,
}

/// 切换结果。
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "result", rename_all = "camelCase")]
pub enum SwitchResult {
    Applied,
    RevertedByTimeout,
    RevertedByUser,
}

/// EDID 备份信息。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub id: String,
    pub monitor_short: String,
    pub created_at: i64,
    pub path: String,
    pub size: usize,
}

/// 统一错误类型（display.md §十五）：serde tag=code，前端精确渲染。
#[derive(Debug, thiserror::Error, Serialize)]
#[serde(tag = "code", content = "detail")]
pub enum QrError {
    #[error("此功能需要 Pro 版")]
    FeatureLocked,
    #[error("当前平台不支持分辨率快速切换（仅 Windows）")]
    UnsupportedPlatform,
    #[error("未检测到 NVIDIA 驱动接口")]
    NvApiUnavailable,
    #[error("驱动已禁用自定义分辨率（DSC 启用），将改用 EDID 注入")]
    NvapiBlockedByDsc,
    #[error("超出链路带宽：需 {need:.1} Gbps，可用 {have:.1} Gbps")]
    BandwidthExceeded { need: f32, have: f32 },
    #[error("超出显示器像素时钟上限 {limit_khz} kHz")]
    ExceedsMonitorLimit { limit_khz: u32 },
    #[error("需要一次管理员授权以启用 EDID 注入")]
    HelperNotInstalled,
    #[error("管理员授权被拒绝")]
    ElevationDenied,
    #[error("辅助进程通信失败：{0}")]
    HelperIpc(String),
    #[error("EDID 无可用时序槽位")]
    EdidNoSlot,
    #[error("未能确定 EDID 覆盖的生效方式，需注销或重启")]
    ActivationRequiresLogoff,
    #[error("预置验证失败，已自动还原（尝试 {attempted} 个模式）")]
    ProvisionVerifyFailed { attempted: usize },
    #[error("该模式尚未预置")]
    ModeNotReady,
    #[error("系统模式列表中不存在该模式")]
    ModeNotRegistered,
    #[error("检测到全屏独占程序，已阻止操作：{process}")]
    BlockedByFullscreenApp { process: String },
    #[error("Win32 调用 {api} 失败，code={code}")]
    Win32 { api: String, code: i32 },
    #[error("超时未确认，已自动回滚")]
    AutoReverted,
    #[error("未找到目标显示器：{0}")]
    DisplayNotFound(String),
    #[error("模式不存在：{0}")]
    ModeNotFound(String),
    #[error("参数非法：{0}")]
    BadRequest(String),
    #[error("EDID 解析/编辑失败：{0}")]
    Edid(String),
    #[error("IO 失败：{0}")]
    Io(String),
}

impl From<qr_edid::EdidErr> for QrError {
    fn from(e: qr_edid::EdidErr) -> Self {
        match e {
            qr_edid::EdidErr::NoSlot => QrError::EdidNoSlot,
            other => QrError::Edid(other.to_string()),
        }
    }
}

impl From<std::io::Error> for QrError {
    fn from(e: std::io::Error) -> Self {
        QrError::Io(e.to_string())
    }
}

/// 当前 Unix 秒。
pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_default() {
        let s = QuickResolutionSettings::default();
        assert!(!s.enabled);
        assert!(s.confirm_before_apply);
        assert_eq!(s.auto_revert_seconds, 15);
        assert!(!s.allow_edid_override);
        assert_eq!(s.schema_version, 1);
    }

    #[test]
    fn settings_serde_roundtrip_camelcase() {
        let s = QuickResolutionSettings::default();
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"schemaVersion\":1"));
        assert!(json.contains("\"maxTrayItems\":8"));
        assert!(json.contains("\"autoRevertSeconds\":15"));
        let back: QuickResolutionSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.schema_version, 1);
    }

    #[test]
    fn mode_state_machine_helpers() {
        assert!(ModeState::Ready.is_ready());
        assert!(ModeState::Active.is_ready());
        assert!(!ModeState::Draft.is_ready());
        assert!(ModeState::Validated.is_pending());
        assert!(ModeState::Failed.is_pending());
        assert!(!ModeState::Ready.is_pending());
    }

    #[test]
    fn mode_entry_brief() {
        let m = DisplayModeEntry {
            id: "m1".into(),
            label: "竞技 4:3".into(),
            width: 1920,
            height: 1440,
            refresh_hz: 480,
            bit_depth: Some(10),
            color_format: Some(ColorFormat::RGB),
            scaling: None,
            target: ModeTarget::Primary,
            timing_standard: TimingStandardKind::Auto,
            manual_timing: None,
            state: ModeState::Ready,
            provision_path: Some(ProvisionPath::Edid),
            last_error: None,
            pinned_to_tray: true,
            order: 0,
            hotkey: None,
            skip_confirm: false,
            created_at: 0,
            last_used_at: None,
        };
        assert_eq!(m.brief(), "1920×1440 @480Hz");
        let json = serde_json::to_string(&m).unwrap();
        assert!(json.contains("\"refreshHz\":480"));
        assert!(json.contains("\"timingStandard\":\"auto\""));
    }

    #[test]
    fn target_serde() {
        let t = ModeTarget::Index { index: 2 };
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(json, r#"{"kind":"index","index":2}"#);
        let back: ModeTarget = serde_json::from_str(&json).unwrap();
        assert_eq!(back, t);
    }

    #[test]
    fn qr_error_serializes_with_code() {
        let e = QrError::FeatureLocked;
        let json = serde_json::to_string(&e).unwrap();
        assert_eq!(json, r#"{"code":"FeatureLocked"}"#);
        let e2 = QrError::BandwidthExceeded { need: 42.4, have: 25.9 };
        let j2 = serde_json::to_string(&e2).unwrap();
        assert!(j2.contains("\"code\":\"BandwidthExceeded\""));
    }

    #[test]
    fn old_json_without_new_fields_loads() {
        // 向后兼容：空 JSON 也能落入 default。
        let s: QuickResolutionSettings = serde_json::from_str(r#"{"schemaVersion":1}"#).unwrap();
        assert!(!s.enabled);
        assert_eq!(s.max_tray_items, 8);
    }
}
