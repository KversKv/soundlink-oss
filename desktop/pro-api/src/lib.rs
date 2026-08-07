//! SoundLink Pro 能力边界（open-core 接口层）。
//!
//! 本 crate **只包含 trait 与数据类型，不含任何业务逻辑**。
//! 实现方有两份同名 crate `soundlink-pro`：
//! - 公开仓库 `desktop/pro/`：免费实现（真实合理的降级行为，不是空占位）；
//! - 私有仓库：Pro 实现（PRO-1 ~ PRO-5 真实逻辑）。
//!
//! 依赖方向：`soundlink → soundlink-pro → soundlink-pro-api`（不可逆）。
//! 设计要点：**没有任何方法叫 `is_pro()`**。所有 Pro 差异都表达为「能力参数」，
//! 业务代码只按能力值行事（工程红线 E4/E5，见 docs/NewFunctions/monetization/01-engineering-plan.md）。

use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// 授权级别。
///
/// 由 `soundlink` 的 license 模块验签后写入 [`EntitlementHandle`]，
/// `soundlink-pro` 实现读取它来决定能力值。免费实现完全忽略该句柄。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Entitlement {
    Free,
    Pro,
}

/// 授权级别共享句柄（AppState 与 Pro 实现共享同一份真相源）。
pub type EntitlementHandle = Arc<parking_lot::RwLock<Entitlement>>;

/// 应用角色（与 `soundlink` 的 Role 对应，此处独立定义以避免反向依赖）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Role {
    #[default]
    Receiver,
    Sender,
}

/// 启动自动化判定输入（由 `soundlink` 从 AppConfig / TrustStore 提取）。
#[derive(Debug, Clone, Default)]
pub struct AutomationInput {
    /// 配置中的「开机自启动」开关。
    pub auto_start: bool,
    /// 配置中的「启动后自动开启接收」开关。
    pub auto_receive_on_start: bool,
    /// 配置中的「启动后自动开启发送」开关。
    pub auto_send_on_start: bool,
    /// 当前角色。
    pub role: Role,
    /// 本次是否由系统自启动拉起（命令行带 `--autostarted`）。
    pub launched_via_autostart: bool,
    /// 可自动连接的上次对端设备（已按 trust store 校验过 host/port 齐全）。
    pub last_peer_device_id: Option<String>,
}

/// 启动时应自动执行的计划。
///
/// 全 `false` / `None` 时实现应返回 `None` 而不是全否定的 `Some`。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartupPlan {
    /// 静默启动：窗口保持隐藏，最小化到托盘。
    pub silent: bool,
    /// 自动开始接收。
    pub auto_receive: bool,
    /// 自动连接指定设备并开始发送（对端 device_id）。
    pub auto_send_to: Option<String>,
}

impl StartupPlan {
    /// 是否不含任何自动化**动作**（实现据此决定返回 `None`）。
    ///
    /// `silent` 只是修饰（如何呈现窗口），本身不是动作：
    /// 没有自动接收/自动发送时，单纯隐藏窗口只会让用户找不到应用，属负体验，
    /// 因此 `silent=true` 而无动作视为空计划（返回 `None`）。
    pub fn is_empty(&self) -> bool {
        !self.auto_receive && self.auto_send_to.is_none()
    }
}

/// 跨启动自动重连的退避策略（PRO-2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReconnectPolicy {
    /// 首次重试延迟（毫秒）。
    pub initial_delay_ms: u64,
    /// 退避倍数 ×100（如 200 = 每次翻倍）。
    pub backoff_factor_x100: u32,
    /// 重试延迟上限（毫秒）。
    pub max_delay_ms: u64,
}

/// 配置档能力（PRO-4）。免费实现不支持多档，`ProCapabilities::profiles()` 返回 `None`。
pub trait ProfileStore: Send + Sync {
    /// 配置档数量上限。
    fn max_profiles(&self) -> usize;
}

/// 全局快捷键动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShortcutAction {
    /// 显示主窗口（基本可用性，免费保留）。
    ShowWindow,
    /// 切换接收/发送角色。
    ToggleRole,
    /// 开始/停止接收。
    StartStopReceiver,
    /// 开始/停止发送。
    StartStopSender,
    /// 循环切换输出设备。
    CycleOutputDevice,
    /// 静音切换。
    ToggleMute,
}

/// 一条快捷键绑定（accelerator 形如 `Ctrl+Shift+S`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShortcutBinding {
    pub accelerator: String,
    pub action: ShortcutAction,
}

/// 托盘直控菜单项（在「显示主窗口 / 设置 / 退出」基础项之外追加）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayItem {
    /// 开始/停止接收（文字随状态翻转）。
    StartStopReceiver,
    /// 开始/停止发送（文字随状态翻转）。
    StartStopSender,
    /// 静音切换。
    ToggleMute,
    /// 「切换到配置档 →」子菜单。
    ProfileSwitcher,
}

/// Pro 能力边界。免费实现返回受限行为，Pro 实现来自私有 crate。
///
/// 业务代码只按本 trait 返回的能力值行事，禁止在 `soundlink` 中判断授权级别。
pub trait ProCapabilities: Send + Sync {
    /// 可记忆的对端设备上限（每个方向独立计数：信任的发送端 / 信任的接收端各计）。
    fn max_remembered_devices(&self) -> usize;

    /// 启动时自动进入的模式（`None` = 不自动）。
    fn startup_plan(&self, input: &AutomationInput) -> Option<StartupPlan>;

    /// 「开机自启动」是否可配置。**对所有用户免费**（属基本可用性，跟随系统）。
    /// 免费实现返回 `true`；此能力恒可用，不随授权变化。
    fn autostart_available(&self) -> bool {
        true
    }

    /// 「启动后自动开启接收/发送」是否可配置（Pro 能力）。
    /// 用于设置页这两个开关的写入门控与置灰展示。
    /// 注意：不含「开机自启动」——后者免费，见 [`ProCapabilities::autostart_available`]。
    fn automation_available(&self) -> bool;

    /// 跨启动自动重连策略（`None` = 不提供跨启动重连）。
    /// 注意：会话内断线重连属流转本体鲁棒性，不在此列（永远免费）。
    fn reconnect_policy(&self) -> Option<ReconnectPolicy>;

    /// 配置档能力（`None` = 不支持多档）。
    fn profiles(&self) -> Option<&dyn ProfileStore>;

    /// 需注册的全局快捷键。
    ///
    /// `custom` 为用户自定义绑定（Pro 可覆盖默认绑定）；免费实现忽略 `custom`，
    /// 仅返回「显示主窗口」一项。
    fn shortcuts(&self, custom: &[ShortcutBinding]) -> Vec<ShortcutBinding>;

    /// 托盘直控菜单项（追加在基础项之后；免费实现返回空）。
    fn tray_items(&self) -> Vec<TrayItem>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_plan_empty_detection() {
        assert!(StartupPlan::default().is_empty());
        // silent 只是修饰、不算动作：仅 silent 视为空计划。
        let p = StartupPlan {
            silent: true,
            ..Default::default()
        };
        assert!(p.is_empty());
        let p = StartupPlan {
            auto_receive: true,
            ..Default::default()
        };
        assert!(!p.is_empty());
        let p = StartupPlan {
            auto_send_to: Some("dev-1".into()),
            ..Default::default()
        };
        assert!(!p.is_empty());
        // silent + 动作：非空（既有动作又有静默呈现）。
        let p = StartupPlan {
            silent: true,
            auto_receive: true,
            ..Default::default()
        };
        assert!(!p.is_empty());
    }

    #[test]
    fn entitlement_handle_shared_mutation() {
        let h: EntitlementHandle = Arc::new(parking_lot::RwLock::new(Entitlement::Free));
        assert_eq!(*h.read(), Entitlement::Free);
        *h.write() = Entitlement::Pro;
        assert_eq!(*h.read(), Entitlement::Pro);
    }

    #[test]
    fn shortcut_binding_serde_roundtrip() {
        let b = ShortcutBinding {
            accelerator: "Ctrl+Shift+P".into(),
            action: ShortcutAction::ToggleRole,
        };
        let json = serde_json::to_string(&b).unwrap();
        let back: ShortcutBinding = serde_json::from_str(&json).unwrap();
        assert_eq!(b, back);
    }

    #[test]
    fn startup_plan_serde_roundtrip() {
        let p = StartupPlan {
            silent: true,
            auto_receive: false,
            auto_send_to: Some("dev-9".into()),
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: StartupPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }
}
