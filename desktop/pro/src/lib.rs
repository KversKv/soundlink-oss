//! SoundLink Pro 能力 · **官方实现（闭源）**。
//!
//! 本 crate 名与公开仓库 `desktop/pro/` 的免费实现完全相同；发布构建时检出覆盖
//! 该目录（junction 或 clone），构建命令不变。见 02-multi-repo-guide §3。
//!
//! 能力语义：授权为 Pro 时提供完整能力；未激活（Free）时返回与免费实现一致的
//! 受限值——官方产物线只有一条，「未激活时行为完全等同免费版」。
//!
//! 激活即时生效：`soundlink` 在 activate/deactivate 时写入共享 `EntitlementHandle`，
//! 本实现每次调用都读该句柄，无需重启（E5：只在命令边界读，不进音频热路径）。

use soundlink_pro_api::{
    AutomationInput, Entitlement, EntitlementHandle, ProCapabilities, ProfileStore,
    ReconnectPolicy, ShortcutAction, ShortcutBinding, StartupPlan, TrayItem,
};
use std::sync::Arc;

/// 构建形态标识。**仅用于日志与 UI 文案**（如 `get_license_status` 的 `pro_build`），
/// 禁止用于门控判断（G6）——门控只看 `ProCapabilities` 返回值。
pub const EDITION: &str = "official";

/// 设备记忆上限（每方向）。
const MAX_REMEMBERED_DEVICES: usize = 8;
/// 配置档上限。
const MAX_PROFILES: usize = 8;
/// 设备记忆上限（免费档回落值）。
const FREE_DEVICE_CAP: usize = 1;

/// Pro 能力实现。持有与 AppState 共享的授权句柄。
struct ProImpl {
    entitlement: EntitlementHandle,
    profile_store: ProProfileStore,
}

struct ProProfileStore {
    max: usize,
}

impl ProImpl {
    fn is_pro(&self) -> bool {
        *self.entitlement.read() == Entitlement::Pro
    }

}

impl ProfileStore for ProProfileStore {
    fn max_profiles(&self) -> usize {
        self.max
    }
}

impl ProCapabilities for ProImpl {
    fn max_remembered_devices(&self) -> usize {
        if self.is_pro() {
            MAX_REMEMBERED_DEVICES
        } else {
            FREE_DEVICE_CAP
        }
    }

    /// 启动计划（PRO-1 + PRO-2）：
    /// - 仅在授权 Pro 时返回计划；未激活恒 None（与免费实现一致）。
    /// - `launched_via_autostart` 且任一自动开关开启 → silent（窗口不弹出，S5）。
    /// - 按角色自动进入接收/发送；发送目标为上次设备（S8/S9，启动即重连）。
    fn startup_plan(&self, input: &AutomationInput) -> Option<StartupPlan> {
        if !self.is_pro() {
            return None;
        }
        let automation_on = input.auto_start
            && (input.auto_receive_on_start || input.auto_send_on_start);
        if !automation_on {
            return None;
        }
        let silent = input.launched_via_autostart;
        let mut plan = StartupPlan {
            silent,
            auto_receive: false,
            auto_send_to: None,
        };
        match input.role {
            soundlink_pro_api::Role::Receiver => {
                if input.auto_receive_on_start {
                    plan.auto_receive = true;
                }
            }
            soundlink_pro_api::Role::Sender => {
                if input.auto_send_on_start {
                    // PRO-2：启动即自动重连上次设备（目标由调用方保证仍在信任存储）。
                    plan.auto_send_to = input.last_peer_device_id.clone();
                }
            }
        }
        // 无任何自动动作时不返回计划（即使 silent：没有要自动化的事，
        // 单纯隐藏窗口只会让用户找不到应用，属负体验）。
        if plan.is_empty() {
            None
        } else {
            Some(plan)
        }
    }

    /// 「开机自启动」对所有用户免费（默认 trait 实现已返回 true）。
    fn autostart_available(&self) -> bool {
        true
    }

    /// 「启动后自动开启接收/发送」开关是否可配置（S4 写入门控 / S6 前端置灰）。
    fn automation_available(&self) -> bool {
        self.is_pro()
    }

    /// 跨启动自动重连策略（PRO-2）：指数退避 1s→30s 上限，静默重试。
    /// 注意：会话内断线重连在 soundlink 核心里，对所有用户开放（不收费）。
    fn reconnect_policy(&self) -> Option<ReconnectPolicy> {
        if !self.is_pro() {
            return None;
        }
        Some(ReconnectPolicy {
            initial_delay_ms: 1_000,
            backoff_factor_x100: 200,
            max_delay_ms: 30_000,
        })
    }

    fn profiles(&self) -> Option<&dyn ProfileStore> {
        if self.is_pro() {
            Some(&self.profile_store)
        } else {
            None
        }
    }

    /// 快捷键（PRO-5）：Pro 下返回完整动作集；用户自定义绑定覆盖同动作的默认键位。
    /// 未激活时与免费实现一致（仅 Ctrl+Shift+S 显示主窗口）。
    fn shortcuts(&self, custom: &[ShortcutBinding]) -> Vec<ShortcutBinding> {
        if !self.is_pro() {
            return vec![ShortcutBinding {
                accelerator: "Ctrl+Shift+S".into(),
                action: ShortcutAction::ShowWindow,
            }];
        }
        let mut defaults = vec![
            ShortcutBinding {
                accelerator: "Ctrl+Shift+S".into(),
                action: ShortcutAction::ShowWindow,
            },
            ShortcutBinding {
                accelerator: "Ctrl+Shift+P".into(),
                action: ShortcutAction::ToggleRole,
            },
            ShortcutBinding {
                accelerator: "Ctrl+Shift+R".into(),
                action: ShortcutAction::StartStopReceiver,
            },
            ShortcutBinding {
                accelerator: "Ctrl+Shift+T".into(),
                action: ShortcutAction::StartStopSender,
            },
            ShortcutBinding {
                accelerator: "Ctrl+Shift+D".into(),
                action: ShortcutAction::CycleOutputDevice,
            },
            ShortcutBinding {
                accelerator: "Ctrl+Shift+M".into(),
                action: ShortcutAction::ToggleMute,
            },
        ];
        // 自定义绑定覆盖同动作默认键位；冲突（同键位不同动作）以自定义为准。
        for c in custom {
            defaults.retain(|d| d.action != c.action && d.accelerator != c.accelerator);
            defaults.push(c.clone());
        }
        defaults
    }

    /// 托盘直控（PRO-5）：追加在「显示主窗口 / 设置 / 退出」基础项之后。
    fn tray_items(&self) -> Vec<TrayItem> {
        if !self.is_pro() {
            return Vec::new();
        }
        vec![
            TrayItem::StartStopReceiver,
            TrayItem::StartStopSender,
            TrayItem::ToggleMute,
            TrayItem::ProfileSwitcher,
        ]
    }

    /// 分辨率快速切换（QR-1）：Pro 授权可用；未激活与免费实现一致（不可用）。
    fn quick_resolution_available(&self) -> bool {
        self.is_pro()
    }
}

/// 工厂：构造能力对象。签名与免费实现完全一致（`soundlink` 不感知差异）。
pub fn capabilities(entitlement: EntitlementHandle) -> Arc<dyn ProCapabilities> {
    Arc::new(ProImpl {
        entitlement,
        profile_store: ProProfileStore { max: MAX_PROFILES },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use soundlink_pro_api::Role;

    fn handle(ent: Entitlement) -> EntitlementHandle {
        Arc::new(parking_lot::RwLock::new(ent))
    }

    fn input(role: Role) -> AutomationInput {
        AutomationInput {
            auto_start: true,
            auto_receive_on_start: true,
            auto_send_on_start: true,
            role,
            launched_via_autostart: true,
            last_peer_device_id: Some("dev-1".into()),
        }
    }

    #[test]
    fn edition_is_official() {
        assert_eq!(EDITION, "official");
    }

    #[test]
    fn free_entitlement_matches_free_impl() {
        let c = capabilities(handle(Entitlement::Free));
        assert_eq!(c.max_remembered_devices(), 1);
        assert_eq!(c.startup_plan(&input(Role::Receiver)), None);
        assert!(!c.automation_available());
        assert_eq!(c.reconnect_policy(), None);
        assert!(c.profiles().is_none());
        let sc = c.shortcuts(&[]);
        assert_eq!(sc.len(), 1);
        assert_eq!(sc[0].action, ShortcutAction::ShowWindow);
        assert!(c.tray_items().is_empty());
    }

    #[test]
    fn pro_entitlement_full_capabilities() {
        let c = capabilities(handle(Entitlement::Pro));
        assert_eq!(c.max_remembered_devices(), 8);
        assert!(c.automation_available());
        assert_eq!(c.profiles().unwrap().max_profiles(), 8);
        assert!(c.reconnect_policy().is_some());
        assert_eq!(c.shortcuts(&[]).len(), 6);
        assert_eq!(c.tray_items().len(), 4);
    }

    #[test]
    fn startup_plan_receiver_silent() {
        let c = capabilities(handle(Entitlement::Pro));
        let plan = c.startup_plan(&input(Role::Receiver)).unwrap();
        assert!(plan.silent);
        assert!(plan.auto_receive);
        assert_eq!(plan.auto_send_to, None);
    }

    #[test]
    fn startup_plan_sender_reconnects_last_peer() {
        let c = capabilities(handle(Entitlement::Pro));
        let plan = c.startup_plan(&input(Role::Sender)).unwrap();
        assert!(plan.silent);
        assert!(!plan.auto_receive);
        assert_eq!(plan.auto_send_to, Some("dev-1".into()));
    }

    #[test]
    fn startup_plan_manual_launch_not_silent() {
        let c = capabilities(handle(Entitlement::Pro));
        let mut i = input(Role::Receiver);
        i.launched_via_autostart = false;
        let plan = c.startup_plan(&i).unwrap();
        assert!(!plan.silent);
        assert!(plan.auto_receive);
    }

    #[test]
    fn startup_plan_none_when_automation_off() {
        let c = capabilities(handle(Entitlement::Pro));
        let i = AutomationInput::default();
        assert_eq!(c.startup_plan(&i), None);
        // auto_start 关但子开关开：不自动（auto_start 是总闸）。
        let mut i2 = input(Role::Receiver);
        i2.auto_start = false;
        assert_eq!(c.startup_plan(&i2), None);
    }

    #[test]
    fn startup_plan_none_when_sender_has_no_peer() {
        let c = capabilities(handle(Entitlement::Pro));
        let mut i = input(Role::Sender);
        i.last_peer_device_id = None;
        // 无目标可连 → 无发送动作；silent 不构成计划（隐藏窗口却没自动化是负体验）→ None。
        assert_eq!(c.startup_plan(&i), None);
    }

    #[test]
    fn activation_takes_effect_immediately() {
        let h = handle(Entitlement::Free);
        let c = capabilities(h.clone());
        assert_eq!(c.max_remembered_devices(), 1);
        assert!(c.startup_plan(&input(Role::Receiver)).is_none());
        // 激活：写共享句柄，无需重建 caps、无需重启。
        *h.write() = Entitlement::Pro;
        assert_eq!(c.max_remembered_devices(), 8);
        assert!(c.startup_plan(&input(Role::Receiver)).is_some());
        // 反激活：回落免费能力。
        *h.write() = Entitlement::Free;
        assert_eq!(c.max_remembered_devices(), 1);
    }

    #[test]
    fn reconnect_policy_backoff_spec() {
        let c = capabilities(handle(Entitlement::Pro));
        let p = c.reconnect_policy().unwrap();
        assert_eq!(p.initial_delay_ms, 1_000);
        assert_eq!(p.backoff_factor_x100, 200);
        assert_eq!(p.max_delay_ms, 30_000);
    }

    #[test]
    fn custom_shortcut_overrides_default_action() {
        let c = capabilities(handle(Entitlement::Pro));
        let custom = vec![ShortcutBinding {
            accelerator: "Ctrl+Alt+P".into(),
            action: ShortcutAction::ToggleRole,
        }];
        let sc = c.shortcuts(&custom);
        let role_binding = sc
            .iter()
            .find(|b| b.action == ShortcutAction::ToggleRole)
            .unwrap();
        assert_eq!(role_binding.accelerator, "Ctrl+Alt+P");
        // 默认 Ctrl+Shift+P 已被覆盖移除。
        assert!(!sc.iter().any(|b| b.accelerator == "Ctrl+Shift+P"));
    }
}
