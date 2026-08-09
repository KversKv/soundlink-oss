//! 批量预置编排（display.md §7.3 策略 C）+ 三层黑屏保险（§7.4）。
//!
//! 核心：**批量**。把用户全部 Draft/Validated 模式一次性合并注入 EDID，
//! 只重启一次设备（而非每加一个分辨率重启一次）。
//!
//! 黑屏保险三层：
//! - L1 helper Watchdog：预置前 ArmWatchdog(60s)，主进程崩溃/黑屏 → helper 自动还原；
//! - L2 启动自检：`pending_recovery.json` 标记，启动时发现上次未收尾 → 回滚；
//! - L3 离线救援：备份目录内 `restore_*.reg` 与 `qr_helper --restore-all`。

use crate::features::quick_resolution::model::*;
use crate::features::quick_resolution::platform::DisplayBackend;
use crate::features::quick_resolution::store::{RecoveryMarker, Store};
use qr_ipc::{ActivationMethod, HelperRequest, HelperResponse, MonitorKey, RegVariant, RestartTarget};

#[cfg(windows)]
use crate::features::quick_resolution::platform::windows::{direct_admin, helper_client::HelperSession};

/// 提权操作执行器：写/删 override 与设备重启。
/// 看门狗不在此列——它必须常驻 helper（独立进程盯着主进程，崩溃自动还原）。
#[cfg(windows)]
enum PrivilegedOps {
    /// 主进程已是管理员：直写 HKLM / 直重启设备，不经计划任务转发。
    Direct,
    /// 普通权限：经 helper 命名管道转发。
    Helper(HelperSession),
}

#[cfg(windows)]
impl PrivilegedOps {
    /// 按当前进程提权状态选择路径。
    fn connect() -> Result<Self, QrError> {
        if direct_admin::is_elevated() {
            return Ok(Self::Direct);
        }
        Ok(Self::Helper(HelperSession::connect()?))
    }

    fn write_override(&mut self, monitor: &MonitorKey, variant: RegVariant, edid: &[u8]) -> Result<(), QrError> {
        match self {
            Self::Direct => direct_admin::write_override(monitor, variant, edid).map(|_| ()),
            Self::Helper(s) => s
                .call(&HelperRequest::WriteEdidOverride {
                    monitor: monitor.clone(),
                    edid: edid.to_vec(),
                    backup_id: String::new(),
                    variant,
                })
                .map(|_| ()),
        }
    }

    fn remove_override(&mut self, monitor: &MonitorKey, variant: RegVariant) -> Result<(), QrError> {
        match self {
            Self::Direct => direct_admin::remove_override(monitor, variant),
            Self::Helper(s) => s
                .call(&HelperRequest::RemoveEdidOverride { monitor: monitor.clone(), variant })
                .map(|_| ()),
        }
    }

    /// 重启显示器，失败再尝试适配器。返回生效的激活方式。
    fn restart(&mut self, monitor: &MonitorKey) -> ActivationMethod {
        let monitor_ok = match self {
            Self::Direct => direct_admin::restart_monitor(monitor).is_ok(),
            Self::Helper(s) => matches!(
                s.call(&HelperRequest::RestartDevice { target: RestartTarget::Monitor, monitor: monitor.clone() }),
                Ok(HelperResponse::Restarted { .. })
            ),
        };
        if monitor_ok {
            return ActivationMethod::MonitorRestart;
        }
        let adapter_ok = match self {
            Self::Direct => direct_admin::restart_adapter().is_ok(),
            Self::Helper(s) => matches!(
                s.call(&HelperRequest::RestartDevice { target: RestartTarget::Adapter, monitor: monitor.clone() }),
                Ok(HelperResponse::Restarted { .. })
            ),
        };
        if adapter_ok {
            ActivationMethod::AdapterRestart
        } else {
            ActivationMethod::LogoffRequired
        }
    }

    /// 武装看门狗。Direct 模式下也走 helper（独立进程守护，主进程崩溃可还原）。
    fn arm_watchdog(&mut self, seconds: u32, backup_id: &str, monitor: &MonitorKey, variant: RegVariant) -> Result<(), QrError> {
        self.helper_session()?
            .call(&HelperRequest::ArmWatchdog {
                seconds,
                backup_id: backup_id.to_string(),
                monitor: monitor.clone(),
                variant,
            })
            .map(|_| ())
    }

    fn disarm_watchdog(&mut self) -> Result<(), QrError> {
        self.helper_session()?
            .call(&HelperRequest::DisarmWatchdog)
            .map(|_| ())
    }

    /// 取 helper 会话：Direct 模式下按需建立（仅用于看门狗）。
    fn helper_session(&mut self) -> Result<&mut HelperSession, QrError> {
        if matches!(self, Self::Direct) {
            // Direct 直写保留看门狗：临时建立 helper 会话仅用于武装/解除。
            *self = Self::Helper(HelperSession::connect()?);
        }
        match self {
            Self::Helper(s) => Ok(s),
            Self::Direct => unreachable!("已转换为 Helper"),
        }
    }
}
use std::sync::Arc;

/// 批量预置（串行锁由 service 层持有，本函数假设已独占）。
pub async fn provision_batch(
    backend: &Arc<dyn DisplayBackend>,
    store: &Store,
    monitor: &MonitorKey,
    gdi_name: &str,
    pending: &[DisplayModeEntry],
) -> Result<ProvisionReport, QrError> {
    if pending.is_empty() {
        return Err(QrError::BadRequest("没有待预置模式".into()));
    }
    // 前置守卫：全屏独占程序禁止（§7.3 step 0）。
    if let Some(proc_name) = backend.fullscreen_exclusive_active() {
        return Err(QrError::BlockedByFullscreenApp { process: proc_name });
    }

    // 1) 读原始 EDID + 备份（强制）。
    let original = backend.read_edid(monitor)?;
    let variant = RegVariant::MonitorInstanceOverride;
    let reg_path = crate::features::quick_resolution::platform::windows::edid_reg::variant_full_path_for_reg(
        &monitor.instance_path,
        variant,
    );
    let backup_id = store.backup_edid(monitor, &original, &reg_path)?;

    // 2) L2：写恢复标记（预置期间存在，收尾删除）。
    store.write_recovery_marker(&RecoveryMarker {
        backup_id: backup_id.clone(),
        monitor: monitor.clone(),
        variant,
        started_at: now_secs(),
        mode_ids: pending.iter().map(|m| m.id.clone()).collect(),
    })?;

    // 3) 合并注入全部 timing 到 EDID。
    let edid_info = qr_edid::EdidDoc::parse(&original).ok().map(|d| d.info());
    let native = edid_info
        .as_ref()
        .and_then(|i| qr_edid::parse::native_timing(i).copied());
    // 显示器行频上限（range limits）：驱动按此裁剪自定义模式，超限必失败。
    let max_h = edid_info.as_ref().and_then(|i| i.max_h_freq_khz);
    let mut doc = qr_edid::EdidDoc::parse(&original)?;
    let mut placed_ids = Vec::new();
    for m in pending {
        let standard = match m.timing_standard {
            TimingStandardKind::Auto => qr_edid::timing::TimingStandard::Auto,
            TimingStandardKind::CvtRb2 => qr_edid::timing::TimingStandard::CvtRb2,
            TimingStandardKind::CvtRb3 => qr_edid::timing::TimingStandard::CvtRb3,
            TimingStandardKind::Manual => {
                let mt = m.manual_timing.ok_or(QrError::BadRequest("手动 timing 缺参数".into()))?;
                qr_edid::timing::TimingStandard::Manual(qr_edid::timing::TimingParams {
                    h_active: m.width,
                    v_active: m.height,
                    h_front: mt.h_front,
                    h_sync: mt.h_sync,
                    h_back: mt.h_back,
                    v_front: mt.v_front,
                    v_sync: mt.v_sync,
                    v_back: mt.v_back,
                    h_sync_pol: mt.h_sync_pol,
                    v_sync_pol: mt.v_sync_pol,
                    interlaced: false,
                })
            }
        };
        let t = qr_edid::timing::generate_for_display(standard, m.width, m.height, m.refresh_hz, native.as_ref(), max_h)?;
        let slot = match doc.insert_timing(&t, m.refresh_hz) {
            Ok(s) => Some(s),
            Err(qr_edid::EdidErr::NoSlot) => {
                // 容量不足：追加 DisplayID 2.0 扩展块再试。
                doc.append_displayid_block()?;
                Some(doc.insert_timing(&t, m.refresh_hz)?)
            }
            Err(e) => return Err(QrError::from(e)),
        };
        crate::features::quick_resolution::helper_core::audit::log(
            "PROVISION-TIMING",
            &format!(
                "{}x{}@{} total={}x{} pclk={}kHz hfreq={:.1}kHz slot={:?}",
                m.width, m.height, m.refresh_hz, t.h_total(), t.v_total(),
                t.pixel_clock_khz(m.refresh_hz), t.h_freq_khz(m.refresh_hz), slot
            ),
        );
        placed_ids.push(m.id.clone());
    }
    doc.fix_extension_count();
    doc.recompute_all_checksums();
    let new_edid = doc.to_bytes();

    // 4) 提权执行器：管理员直写 或 经 helper 转发（按进程提权状态自动选择）。
    let mut ops = PrivilegedOps::connect().map_err(|e| {
        store.clear_recovery_marker();
        e
    })?;

    // 写 override
    ops.write_override(monitor, variant, &new_edid)?;

    // L1：武装看门狗（60s 内未 disarm → helper 自动还原并重启）。
    // 直写模式下也经 helper（独立进程守护，主进程崩溃可还原）。
    ops.arm_watchdog(60, &backup_id, monitor, variant)?;

    // 5) 激活方式阶梯：Monitor 重启 → Adapter 重启。
    let activation = ops.restart(monitor);

    // 6) 验证闭环：模式是否进入系统列表。
    let sys_modes = backend.enum_modes(gdi_name)?;
    let (ok_modes, fail_modes): (Vec<&DisplayModeEntry>, Vec<&DisplayModeEntry>) =
        pending.iter().partition(|m| sys_modes.iter().any(|s| s.matches(m)));
    let ok_ids: Vec<String> = ok_modes.iter().map(|m| m.id.clone()).collect();
    let fail_ids: Vec<String> = fail_modes.iter().map(|m| m.id.clone()).collect();

    if ok_ids.is_empty() {
        // 全部失败：自动回滚 EDID + 重启设备。
        let _ = ops.remove_override(monitor, variant);
        let _ = ops.restart(monitor);
        let _ = ops.disarm_watchdog();
        store.clear_recovery_marker();
        return Err(QrError::ProvisionVerifyFailed { attempted: pending.len() });
    }

    // 7) 成功：解除看门狗 + 清恢复标记。
    ops.disarm_watchdog()?;
    store.clear_recovery_marker();

    Ok(ProvisionReport {
        succeeded: ok_ids,
        failed: fail_ids,
        activation: format!("{:?}", activation),
        backup_id,
    })
}

/// L2 启动自检：发现上次预置未收尾 → 回滚 EDID。
///
/// 由 main.rs setup 调用；helper 已装时静默执行（免 UAC）。
pub fn startup_recovery_check(store: &Store) -> Option<String> {
    let marker = store.read_recovery_marker()?;
    tracing::warn!("QR 启动自检：发现上次预置未收尾（backup={}），回滚 EDID", marker.backup_id);
    #[cfg(windows)]
    {
        if let Ok(mut session) =
            crate::features::quick_resolution::platform::windows::helper_client::HelperSession::connect()
        {
            let _ = session.call(&HelperRequest::RemoveEdidOverride {
                monitor: marker.monitor.clone(),
                variant: marker.variant,
            });
            let _ = session.call(&HelperRequest::RestartDevice {
                target: RestartTarget::Monitor,
                monitor: marker.monitor.clone(),
            });
        }
    }
    store.clear_recovery_marker();
    Some(marker.backup_id)
}
