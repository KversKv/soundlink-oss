//! 持久化（display.md §九「持久化布局」）。
//!
//! ```text
//! <config_dir>/
//! ├── quick_resolution.json        # 设置 + 模式列表（schemaVersion 迁移）
//! ├── capability_profiles.json     # 能力档案缓存
//! ├── pending_recovery.json        # 崩溃恢复标记（仅预置期间存在）
//! └── backups/edid/                # EDID 备份 + 一键还原 .reg
//! ```

use crate::features::quick_resolution::model::{
    now_secs, CapabilityProfile, QrError, QuickResolutionSettings,
};
use qr_ipc::MonitorKey;
use std::fs;
use std::path::{Path, PathBuf};

const SETTINGS_FILE: &str = "quick_resolution.json";
const PROFILES_FILE: &str = "capability_profiles.json";
const RECOVERY_FILE: &str = "pending_recovery.json";
const BACKUP_DIR: &str = "backups/edid";

/// 设置读写。
pub struct Store {
    dir: PathBuf,
}

impl Store {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    /// 加载设置；不存在/损坏 → 默认值（损坏时保留 .bad 副本）。
    pub fn load_settings(&self) -> QuickResolutionSettings {
        match fs::read_to_string(self.path(SETTINGS_FILE)) {
            Ok(text) => match serde_json::from_str::<QuickResolutionSettings>(&text) {
                Ok(s) if s.schema_version == 1 => s,
                Ok(s) => {
                    // 未来版本迁移入口：当前仅 v1，遇到更高版本保守回退默认。
                    tracing::warn!(
                        "quick_resolution.json schemaVersion={} 不受支持，备份并重置",
                        s.schema_version
                    );
                    let _ = fs::rename(
                        self.path(SETTINGS_FILE),
                        self.path(&format!("{}.bad", SETTINGS_FILE)),
                    );
                    QuickResolutionSettings::default()
                }
                Err(e) => {
                    tracing::warn!("quick_resolution.json 解析失败：{}，备份并重置", e);
                    let _ = fs::rename(
                        self.path(SETTINGS_FILE),
                        self.path(&format!("{}.bad", SETTINGS_FILE)),
                    );
                    QuickResolutionSettings::default()
                }
            },
            Err(_) => QuickResolutionSettings::default(),
        }
    }

    /// 原子写（tmp + rename）。
    pub fn save_settings(&self, s: &QuickResolutionSettings) -> Result<(), QrError> {
        if let Err(e) = fs::create_dir_all(&self.dir) {
            return Err(QrError::Io(format!("创建配置目录失败：{}", e)));
        }
        let tmp = self.path(&format!("{}.tmp", SETTINGS_FILE));
        let text = serde_json::to_string_pretty(s).map_err(|e| QrError::Io(e.to_string()))?;
        fs::write(&tmp, text)?;
        fs::rename(&tmp, self.path(SETTINGS_FILE))?;
        Ok(())
    }

    // ---- 能力档案缓存 ----

    pub fn load_profiles(&self) -> Vec<CapabilityProfile> {
        fs::read_to_string(self.path(PROFILES_FILE))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save_profiles(&self, profiles: &[CapabilityProfile]) -> Result<(), QrError> {
        let text = serde_json::to_string_pretty(profiles).map_err(|e| QrError::Io(e.to_string()))?;
        fs::write(self.path(PROFILES_FILE), text)?;
        Ok(())
    }

    // ---- 崩溃恢复标记（L2 启动自检）----

    /// 预置开始前写入标记；正常收尾后删除。
    pub fn write_recovery_marker(&self, payload: &RecoveryMarker) -> Result<(), QrError> {
        fs::create_dir_all(&self.dir)?;
        let text = serde_json::to_string_pretty(payload).map_err(|e| QrError::Io(e.to_string()))?;
        fs::write(self.path(RECOVERY_FILE), text)?;
        Ok(())
    }

    pub fn read_recovery_marker(&self) -> Option<RecoveryMarker> {
        fs::read_to_string(self.path(RECOVERY_FILE))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
    }

    pub fn clear_recovery_marker(&self) {
        let _ = fs::remove_file(self.path(RECOVERY_FILE));
    }

    // ---- EDID 备份 ----

    pub fn backup_dir(&self) -> PathBuf {
        self.dir.join(BACKUP_DIR)
    }

    /// 备份 EDID（.bin）+ 生成一键还原 .reg（L3 离线救援）。
    /// 返回 backup_id（`<monitorKey8>-<yyyymmdd>-<hhmmss>`）。
    pub fn backup_edid(
        &self,
        monitor: &MonitorKey,
        edid: &[u8],
        reg_variant_path: &str,
    ) -> Result<String, QrError> {
        let dir = self.backup_dir();
        fs::create_dir_all(&dir)?;
        let id = format!("{}-{}", monitor.short(), timestamp_id());
        let bin_path = dir.join(format!("{}.bin", id));
        fs::write(&bin_path, edid)?;

        // .reg 还原文件：双击即可把原 EDID 写回 override 值（安全模式可用）。
        let hex: String = edid.iter().map(|b| format!("{:02X},", b)).collect();
        let reg = format!(
            "Windows Registry Editor Version 5.00\r\n\r\n; SoundLink QR EDID 还原（备份 {}，显示器 {}）\r\n; 双击导入后重启显示驱动或注销重登生效。\r\n[{}]\r\n\"EDID_OVERRIDE\"=hex:{}\r\n",
            id,
            monitor.short(),
            reg_variant_path,
            hex.trim_end_matches(',')
        );
        let reg_path = dir.join(format!("restore_{}.reg", id));
        fs::write(&reg_path, reg)?;
        Ok(id)
    }

    /// 列出备份（按时间倒序）。
    pub fn list_backups(&self, monitor: Option<&MonitorKey>) -> Vec<crate::features::quick_resolution::model::BackupInfo> {
        let dir = self.backup_dir();
        let mut out = Vec::new();
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => return out,
        };
        for e in entries.flatten() {
            let p = e.path();
            let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
            if !name.ends_with(".bin") {
                continue;
            }
            let id = name.trim_end_matches(".bin").to_string();
            let short = id.split('-').next().unwrap_or("").to_string();
            if let Some(m) = monitor {
                if m.short() != short {
                    continue;
                }
            }
            let meta = fs::metadata(&p).ok();
            let created = meta
                .as_ref()
                .and_then(|m| m.created().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            out.push(crate::features::quick_resolution::model::BackupInfo {
                id,
                monitor_short: short,
                created_at: created,
                path: p.to_string_lossy().into_owned(),
                size: meta.map(|m| m.len() as usize).unwrap_or(0),
            });
        }
        out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        out
    }

    /// 按 backup_id 读取备份 EDID。
    pub fn read_backup(&self, backup_id: &str) -> Result<Vec<u8>, QrError> {
        // 防路径穿越：只允许字母数字与 '-'。
        if !backup_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(QrError::BadRequest("backup_id 非法".into()));
        }
        let p = self.backup_dir().join(format!("{}.bin", backup_id));
        Ok(fs::read(p)?)
    }
}

/// 崩溃恢复标记内容（L2：启动自检据此回滚）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryMarker {
    pub backup_id: String,
    pub monitor: MonitorKey,
    pub variant: qr_ipc::RegVariant,
    pub started_at: i64,
    /// 待验证的模式 id 列表。
    pub mode_ids: Vec<String>,
}

/// `20260808-162345` 形式时间戳（本地时间近似：直接用 UTC 秒换算，避免引入 chrono）。
fn timestamp_id() -> String {
    let secs = now_secs().max(0) as u64;
    // 简化：用 days since epoch 推 ymd，秒推 hms（UTC）。
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let (y, m, d) = civil_from_days(days as i64);
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        y,
        m,
        d,
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Howard Hinnant 的 civil_from_days 算法。
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_store() -> (Store, PathBuf) {
        let mut p = std::env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        p.push(format!("soundlink_qr_store_test_{}_{}", std::process::id(), nanos));
        let _ = fs::remove_dir_all(&p);
        (Store::new(p.clone()), p)
    }

    #[test]
    fn settings_roundtrip() {
        let (store, dir) = tmp_store();
        let mut s = QuickResolutionSettings::default();
        assert!(!store.path(SETTINGS_FILE).exists());
        assert_eq!(store.load_settings().schema_version, 1);
        s.enabled = true;
        store.save_settings(&s).unwrap();
        let loaded = store.load_settings();
        assert!(loaded.enabled);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupted_settings_falls_back() {
        let (store, dir) = tmp_store();
        fs::create_dir_all(&dir).unwrap();
        fs::write(store.path(SETTINGS_FILE), "{ not json").unwrap();
        let s = store.load_settings();
        assert_eq!(s.schema_version, 1);
        assert!(store.path(&format!("{}.bad", SETTINGS_FILE)).exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn backup_and_read() {
        let (store, dir) = tmp_store();
        let key = MonitorKey {
            instance_path: "DISPLAY\\ABC\\1".into(),
            edid_hash: "0123456789abcdef".into(),
        };
        let edid = vec![0xAAu8; 128];
        let id = store
            .backup_edid(&key, &edid, "HKEY_LOCAL_MACHINE\\SYSTEM\\X")
            .unwrap();
        assert!(id.starts_with("01234567-"));
        // .bin 与 restore .reg 都应存在
        assert!(store.backup_dir().join(format!("{}.bin", id)).exists());
        assert!(store.backup_dir().join(format!("restore_{}.reg", id)).exists());
        let back = store.read_backup(&id).unwrap();
        assert_eq!(back, edid);
        let list = store.list_backups(Some(&key));
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, id);
        // 路径穿越防御
        assert!(store.read_backup("../evil").is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn recovery_marker_roundtrip() {
        let (store, dir) = tmp_store();
        assert!(store.read_recovery_marker().is_none());
        let m = RecoveryMarker {
            backup_id: "b1".into(),
            monitor: MonitorKey { instance_path: "a".into(), edid_hash: "b".into() },
            variant: qr_ipc::RegVariant::MonitorInstanceOverride,
            started_at: 1,
            mode_ids: vec!["m1".into()],
        };
        store.write_recovery_marker(&m).unwrap();
        let got = store.read_recovery_marker().unwrap();
        assert_eq!(got.backup_id, "b1");
        store.clear_recovery_marker();
        assert!(store.read_recovery_marker().is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn timestamp_format() {
        let ts = timestamp_id();
        assert_eq!(ts.len(), 15);
        assert_eq!(&ts[8..9], "-");
    }

    #[test]
    fn civil_date_known() {
        // 2026-08-08 = epoch 后第 20673 天（20673×86400+86400=2026-08-09 00:00 UTC）。
        let (y, m, d) = civil_from_days(20_673);
        assert_eq!((y, m, d), (2026, 8, 8));
    }
}
