//! SoundLink Pro 能力 · **免费实现（社区构建）**。
//!
//! 这是真实且合理的降级行为，不是空洞占位（红线 E3）：
//! 免费版就是「设备各记 1 台、不自动启动、无配置档」的完整产品。
//!
//! 官方构建时本目录被私有实现整体替换（crate 名与版本号保持一致），
//! 构建命令完全相同。见 docs/NewFunctions/monetization/02-multi-repo-guide.md。

use soundlink_pro_api::{
    EntitlementHandle, ProCapabilities, ProfileStore, ReconnectPolicy, ShortcutAction,
    ShortcutBinding, StartupPlan, TrayItem, AutomationInput,
};
use std::sync::Arc;

/// 构建形态标识。**仅用于日志与 UI 文案**（如「本构建不含 Pro（社区版）」），
/// 禁止用于门控判断（红线 G6）——门控只看 `ProCapabilities` 返回值。
pub const EDITION: &str = "community";

/// 免费能力集。
///
/// 不读取 `EntitlementHandle`：社区构建不含 Pro 逻辑，任何授权状态都返回免费能力值。
struct FreeCapabilities;

impl ProCapabilities for FreeCapabilities {
    /// 每个方向（信任的发送端 / 信任的接收端）各记 1 台：记住最近用的那台。
    fn max_remembered_devices(&self) -> usize {
        1
    }

    /// 免费版不自动启动任何模式，手动「点开始」即可完成同样的事。
    fn startup_plan(&self, _input: &AutomationInput) -> Option<StartupPlan> {
        None
    }

    /// 「开机自启动」对所有用户免费（默认 trait 实现已返回 true，此处显式固定语义）。
    fn autostart_available(&self) -> bool {
        true
    }

    /// 「启动后自动开启接收/发送」为 Pro 能力，免费版不可配置。
    fn automation_available(&self) -> bool {
        false
    }

    /// 跨启动自动重连不提供；会话内断线重连在 soundlink 核心里，对所有用户开放。
    fn reconnect_policy(&self) -> Option<ReconnectPolicy> {
        None
    }

    fn profiles(&self) -> Option<&dyn ProfileStore> {
        None
    }

    /// 免费版仅保留「显示主窗口」快捷键（找回被最小化的窗口属基本可用性）。
    fn shortcuts(&self, _custom: &[ShortcutBinding]) -> Vec<ShortcutBinding> {
        vec![ShortcutBinding {
            accelerator: "Ctrl+Shift+S".into(),
            action: ShortcutAction::ShowWindow,
        }]
    }

    /// 免费版托盘仅基础项（显示主窗口 / 设置 / 退出），无追加直控项。
    fn tray_items(&self) -> Vec<TrayItem> {
        Vec::new()
    }

    /// 免费版无「分辨率快速切换」（QR-1）。
    fn quick_resolution_available(&self) -> bool {
        false
    }
}

/// 工厂：构造能力对象。
///
/// 签名必须与私有实现完全一致；免费实现忽略授权句柄。
pub fn capabilities(_entitlement: EntitlementHandle) -> Arc<dyn ProCapabilities> {
    Arc::new(FreeCapabilities)
}

#[cfg(test)]
mod tests {
    use super::*;
    use soundlink_pro_api::Entitlement;

    fn caps() -> Arc<dyn ProCapabilities> {
        let h: EntitlementHandle = Arc::new(parking_lot::RwLock::new(Entitlement::Free));
        capabilities(h)
    }

    #[test]
    fn edition_is_community() {
        assert_eq!(EDITION, "community");
    }

    #[test]
    fn remembers_one_device_per_direction() {
        assert_eq!(caps().max_remembered_devices(), 1);
    }

    #[test]
    fn no_startup_plan_even_with_all_flags_on() {
        let input = AutomationInput {
            auto_start: true,
            auto_receive_on_start: true,
            auto_send_on_start: true,
            role: soundlink_pro_api::Role::Receiver,
            launched_via_autostart: true,
            last_peer_device_id: Some("dev-1".into()),
        };
        assert_eq!(caps().startup_plan(&input), None);
    }

    #[test]
    fn automation_not_available() {
        assert!(!caps().automation_available());
    }

    #[test]
    fn no_reconnect_policy() {
        assert_eq!(caps().reconnect_policy(), None);
    }

    #[test]
    fn no_profiles() {
        assert!(caps().profiles().is_none());
    }

    #[test]
    fn only_show_window_shortcut() {
        let sc = caps().shortcuts(&[]);
        assert_eq!(sc.len(), 1);
        assert_eq!(sc[0].accelerator, "Ctrl+Shift+S");
        assert_eq!(sc[0].action, ShortcutAction::ShowWindow);
    }

    #[test]
    fn custom_shortcuts_ignored() {
        let custom = vec![ShortcutBinding {
            accelerator: "Ctrl+Shift+X".into(),
            action: ShortcutAction::ToggleRole,
        }];
        let sc = caps().shortcuts(&custom);
        assert_eq!(sc.len(), 1);
        assert_eq!(sc[0].action, ShortcutAction::ShowWindow);
    }

    #[test]
    fn no_tray_items() {
        assert!(caps().tray_items().is_empty());
    }

    #[test]
    fn entitlement_handle_ignored_even_if_pro() {
        // 社区构建：即使句柄被写成 Pro，能力值仍全部为免费档。
        let h: EntitlementHandle = Arc::new(parking_lot::RwLock::new(Entitlement::Pro));
        let c = capabilities(h);
        assert_eq!(c.max_remembered_devices(), 1);
        assert!(!c.automation_available());
        assert!(c.reconnect_policy().is_none());
    }
}
