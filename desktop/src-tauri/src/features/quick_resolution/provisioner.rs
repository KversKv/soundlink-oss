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
    let native = qr_edid::EdidDoc::parse(&original)
        .ok()
        .and_then(|d| {
            let info = d.info();
            qr_edid::parse::native_timing(&info).copied()
        });
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
        let t = qr_edid::timing::generate(standard, m.width, m.height, m.refresh_hz, native.as_ref())?;
        match doc.insert_timing(&t, m.refresh_hz) {
            Ok(_slot) => placed_ids.push(m.id.clone()),
            Err(qr_edid::EdidErr::NoSlot) => {
                // 容量不足：追加 DisplayID 2.0 扩展块再试。
                doc.append_displayid_block()?;
                doc.insert_timing(&t, m.refresh_hz)?;
                placed_ids.push(m.id.clone());
            }
            Err(e) => return Err(QrError::from(e)),
        }
    }
    doc.fix_extension_count();
    doc.recompute_all_checksums();
    let new_edid = doc.to_bytes();

    // 4) helper：写 override + 武装看门狗。
    let mut session = crate::features::quick_resolution::platform::windows::helper_client::HelperSession::connect()
        .map_err(|e| {
            store.clear_recovery_marker();
            e
        })?;

    // 写 override
    session.call(&HelperRequest::WriteEdidOverride {
        monitor: monitor.clone(),
        edid: new_edid,
        backup_id: backup_id.clone(),
        variant,
    })?;

    // L1：武装看门狗（60s 内未 disarm → helper 自动还原并重启）。
    session.call(&HelperRequest::ArmWatchdog {
        seconds: 60,
        backup_id: backup_id.clone(),
        monitor: monitor.clone(),
        variant,
    })?;

    // 5) 激活方式阶梯：Monitor 重启 → Adapter 重启。
    let activation = match session.call(&HelperRequest::RestartDevice {
        target: RestartTarget::Monitor,
        monitor: monitor.clone(),
    }) {
        Ok(HelperResponse::Restarted { method, .. }) => method,
        _ => {
            // 显示器重启失败 → 适配器重启（代价更大）。
            match session.call(&HelperRequest::RestartDevice {
                target: RestartTarget::Adapter,
                monitor: monitor.clone(),
            }) {
                Ok(HelperResponse::Restarted { method, .. }) => method,
                _ => ActivationMethod::LogoffRequired,
            }
        }
    };

    // 6) 验证闭环：模式是否进入系统列表。
    let sys_modes = backend.enum_modes(gdi_name)?;
    let (ok_modes, fail_modes): (Vec<&DisplayModeEntry>, Vec<&DisplayModeEntry>) =
        pending.iter().partition(|m| sys_modes.iter().any(|s| s.matches(m)));
    let ok_ids: Vec<String> = ok_modes.iter().map(|m| m.id.clone()).collect();
    let fail_ids: Vec<String> = fail_modes.iter().map(|m| m.id.clone()).collect();

    if ok_ids.is_empty() {
        // 全部失败：自动回滚 EDID + 重启设备。
        let _ = session.call(&HelperRequest::RemoveEdidOverride { monitor: monitor.clone(), variant });
        let _ = session.call(&HelperRequest::RestartDevice { target: RestartTarget::Monitor, monitor: monitor.clone() });
        let _ = session.call(&HelperRequest::DisarmWatchdog);
        store.clear_recovery_marker();
        return Err(QrError::ProvisionVerifyFailed { attempted: pending.len() });
    }

    // 7) 成功：解除看门狗 + 清恢复标记。
    session.call(&HelperRequest::DisarmWatchdog)?;
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
