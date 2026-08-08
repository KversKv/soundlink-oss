//! 看门狗（L1 黑屏保险，display.md §7.4）+ 离线救援（L3 `--restore-all`）。
//!
//! - `arm(seconds, ...)`：主进程预置前武装；到期未 disarm → 自动还原备份 EDID + 重启设备。
//! - `disarm()`：验证闭环成功后解除。
//! - `restore_all()`：安全模式下还原备份目录内全部 `.bin`（按实例变体写回 override）。

use crate::features::quick_resolution::model::QrError;
use crate::features::quick_resolution::platform::windows::{device_restart, edid_reg};
use qr_ipc::{MonitorKey, RegVariant, RestartTarget};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// 看门狗状态（helper 进程内单例）。
static ARMED: AtomicBool = AtomicBool::new(false);
static STATE: Mutex<Option<WatchdogState>> = Mutex::new(None);

struct WatchdogState {
    backup_id: String,
    monitor: MonitorKey,
    variant: RegVariant,
    deadline: std::time::Instant,
    backup_dir: std::path::PathBuf,
}

/// 武装看门狗（独立线程计时）。
pub fn arm(
    seconds: u32,
    backup_id: String,
    monitor: MonitorKey,
    variant: RegVariant,
    backup_dir: std::path::PathBuf,
) {
    disarm();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(seconds as u64);
    {
        let mut s = STATE.lock().unwrap();
        *s = Some(WatchdogState { backup_id, monitor, variant, deadline, backup_dir });
    }
    ARMED.store(true, Ordering::SeqCst);
    std::thread::spawn(|| loop {
        std::thread::sleep(std::time::Duration::from_millis(500));
        if !ARMED.load(Ordering::SeqCst) {
            return; // 已解除
        }
        let fire = {
            let s = STATE.lock().unwrap();
            s.as_ref().map(|w| std::time::Instant::now() >= w.deadline).unwrap_or(false)
        };
        if fire {
            ARMED.store(false, Ordering::SeqCst);
            let s = STATE.lock().unwrap().take();
            if let Some(w) = s {
                crate::features::quick_resolution::helper_core::audit::log(
                    "WATCHDOG-FIRE",
                    &format!("超时未确认，自动还原 {}", w.backup_id),
                );
                let _ = restore_from_backup(&w.backup_dir, &w.backup_id, &w.monitor, w.variant);
                let _ = device_restart::restart_device(&w.monitor.instance_path);
            }
            return;
        }
    });
}

/// 解除看门狗。
pub fn disarm() {
    ARMED.store(false, Ordering::SeqCst);
    if let Ok(mut s) = STATE.lock() {
        *s = None;
    }
}

/// 是否处于武装状态。
pub fn is_armed() -> bool {
    ARMED.load(Ordering::SeqCst)
}

/// 从备份还原 EDID（写回 override 值）。
pub fn restore_from_backup(
    backup_dir: &std::path::Path,
    backup_id: &str,
    monitor: &MonitorKey,
    variant: RegVariant,
) -> Result<(), QrError> {
    let path = backup_dir.join(format!("{}.bin", backup_id));
    let edid = std::fs::read(&path)
        .map_err(|e| QrError::Io(format!("读取备份失败 {:?}：{}", path, e)))?;
    crate::features::quick_resolution::helper_core::audit::log(
        "RESTORE",
        &format!("backup={} monitor={} hash={}", backup_id, monitor.short(), crate::features::quick_resolution::helper_core::audit::edid_digest(&edid)),
    );
    edid_reg::write_override(&monitor.instance_path, variant, &edid)?;
    Ok(())
}

/// L3 离线救援：还原备份目录内全部 EDID 备份（安全模式双击执行）。
///
/// 策略：对每份 `.bin`，按 `MonitorKey` 推导的实例变体写回 override 并记录；
/// 找不到对应显示器实例（已拔线）时跳过（备份文件仍在，可手动 .reg 还原）。
pub fn restore_all() -> Result<usize, QrError> {
    let dir = backup_dir_default();
    let mut restored = 0usize;
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Ok(0),
    };
    // 枚举当前在线显示器实例路径。
    let online = crate::features::quick_resolution::platform::windows::ccd::enumerate_displays()
        .map(|ds| ds.into_iter().map(|d| d.key).collect::<Vec<_>>())
        .unwrap_or_default();
    for e in entries.flatten() {
        let p = e.path();
        let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
        if !name.ends_with(".bin") {
            continue;
        }
        // 文件名 = <edid_hash8>-<ts>.bin；匹配在线显示器 edid_hash 前缀。
        let short = name.trim_end_matches(".bin").split('-').next().unwrap_or("").to_string();
        if short.is_empty() {
            continue;
        }
        for key in &online {
            if key.edid_hash.starts_with(&short) || key.short() == short {
                if let Ok(edid) = std::fs::read(&p) {
                    if edid_reg::write_override(
                        &key.instance_path,
                        RegVariant::MonitorInstanceOverride,
                        &edid,
                    )
                    .is_ok()
                    {
                        crate::features::quick_resolution::helper_core::audit::log(
                            "RESTORE-ALL",
                            &format!("{} -> {}", name, key.short()),
                        );
                        restored += 1;
                    }
                }
                break;
            }
        }
    }
    // 删除计划任务（救援后不再自动拉起 helper）。
    let _ = super::scheduled_task::uninstall();
    Ok(restored)
}

/// 备份目录默认位置（与 store.rs 约定一致）。
fn backup_dir_default() -> std::path::PathBuf {
    let mut p = dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    p.push("soundlink");
    p.push("backups");
    p.push("edid");
    p
}

#[allow(dead_code)]
fn _unused(_: RestartTarget) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disarm_idempotent() {
        disarm();
        disarm();
        assert!(!is_armed());
    }
}
