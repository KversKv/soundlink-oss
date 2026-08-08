//! QrService 门面（display.md §三）：串行锁 + 事件广播 + 全部业务编排。
//!
//! 并发约束：
//! - 预置走 `provision_lock` 全局串行（M7）；
//! - 切换确认用 oneshot（`pending_confirm`），同时至多一个在途；
//! - parking_lot 守卫**绝不跨 await** 持有。

use crate::features::quick_resolution::applier::{self, ResolvedDisplay};
use crate::features::quick_resolution::model::*;
use crate::features::quick_resolution::platform::{default_backend, DisplayBackend};
use crate::features::quick_resolution::rollback::RollbackGuard;
use crate::features::quick_resolution::store::Store;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

type BackendArc = Arc<dyn DisplayBackend>;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tokio::sync::oneshot;

/// 切换确认窗尺寸。
const CONFIRM_W: f64 = 380.0;
const CONFIRM_H: f64 = 170.0;

pub struct QrService {
    backend: BackendArc,
    store: Store,
    settings: RwLock<QuickResolutionSettings>,
    /// 预置全局串行锁（M7 使用；M1 预留）。
    #[allow(dead_code)]
    pub(crate) provision_lock: tokio::sync::Mutex<()>,
    /// 在途切换确认（true=确认 / false=回滚）。
    pending_confirm: parking_lot::Mutex<Option<oneshot::Sender<bool>>>,
    /// 托盘「恢复上一个分辨率」：切换前模式。
    last_previous: parking_lot::Mutex<Option<(ModeTarget, SystemMode)>>,
    /// 本会话每块显示器的初始模式（restore_on_app_exit 用）。
    session_originals: parking_lot::Mutex<HashMap<String, SystemMode>>,
    /// 能力档案缓存（M6 使用；M1 预留读写通路）。
    #[allow(dead_code)]
    pub(crate) profiles: RwLock<Vec<CapabilityProfile>>,
}

impl QrService {
    pub fn new(config_dir: PathBuf) -> Arc<Self> {
        let store = Store::new(config_dir);
        let settings = store.load_settings();
        let profiles = store.load_profiles();
        Arc::new(Self {
            backend: Arc::from(default_backend()),
            store,
            settings: RwLock::new(settings),
            provision_lock: tokio::sync::Mutex::new(()),
            pending_confirm: parking_lot::Mutex::new(None),
            last_previous: parking_lot::Mutex::new(None),
            session_originals: parking_lot::Mutex::new(HashMap::new()),
            profiles: RwLock::new(profiles),
        })
    }

    #[allow(dead_code)] // M3/M7 能力探测与预置使用
    pub(crate) fn backend(&self) -> &dyn DisplayBackend {
        self.backend.as_ref()
    }

    #[allow(dead_code)] // M7 预置编排使用
    pub(crate) fn store(&self) -> &Store {
        &self.store
    }

    // ---- 设置 ----

    pub fn settings(&self) -> QuickResolutionSettings {
        self.settings.read().clone()
    }

    /// 标记 helper 已完成安装并持久化（安装命令成功后调用）。
    pub fn mark_helper_installed(&self) {
        let mut s = self.settings.write();
        if !s.helper_installed {
            s.helper_installed = true;
            let saved = s.clone();
            drop(s);
            let _ = self.store.save_settings(&saved);
        }
    }

    /// 全量覆盖设置（前端持有完整对象）；数值字段做夹取校验。
    pub fn save_settings(&self, mut next: QuickResolutionSettings) -> Result<QuickResolutionSettings, QrError> {
        next.schema_version = 1;
        next.auto_revert_seconds = next.auto_revert_seconds.clamp(5, 60);
        next.max_tray_items = next.max_tray_items.clamp(1, 16);
        // 模式按 order 排序落盘，保证托盘/前端顺序一致。
        next.modes.sort_by_key(|m| m.order);
        self.store.save_settings(&next)?;
        *self.settings.write() = next.clone();
        Ok(next)
    }

    #[allow(dead_code)] // M7 预置期间局部持久化使用
    fn persist_current(&self) -> Result<(), QrError> {
        let s = self.settings.read().clone();
        self.store.save_settings(&s)
    }

    // ---- 显示器 ----

    pub fn list_displays(&self) -> Result<Vec<DisplayInfo>, QrError> {
        let mut displays = self.backend.enumerate()?;
        self.fill_dsc(&mut displays);
        Ok(displays)
    }

    /// 填充 DSC 判定 + 链路信息（M3，Windows 生效；非 Windows/无 NVAPI 自动降级 Unknown）。
    fn fill_dsc(&self, displays: &mut [DisplayInfo]) {
        #[cfg(windows)]
        {
            use crate::features::quick_resolution::platform::windows::{dsc, nvapi::NvApi};
            let forced = match self.settings.read().dsc_override {
                DscOverride::Auto => None,
                DscOverride::ForceOn => Some(true),
                DscOverride::ForceOff => Some(false),
            };
            // NVAPI 会话一次加载，多块显示器复用。
            let api_result = NvApi::load();
            let api = api_result.as_ref().ok();
            let handles = api.map(|a| a.display_handles()).unwrap_or_default();
            for (i, d) in displays.iter_mut().enumerate() {
                let mut debug: Vec<String> = Vec::new();
                if let Err(e) = &api_result {
                    debug.push(format!("NVAPI 加载失败：{}", e));
                }
                if api.is_some() && handles.is_empty() {
                    debug.push("NVAPI 枚举显示句柄为空".into());
                }
                let link = api.and_then(|a| {
                    handles.get(i).and_then(|h| {
                        a.link_info(*h)
                            .map_err(|e| {
                                debug.push(format!("link_info 失败：{}", e));
                            })
                            .ok()
                    })
                });
                if api.is_some() && link.is_none() && !handles.is_empty() {
                    debug.push("link_info 返回 Err（驱动不支持该字段）".into());
                }
                let edid_dsc = match self.backend.read_edid(&d.key) {
                    Ok(e) => dsc::edid_dsc_support(&e),
                    Err(e) => {
                        debug.push(format!("EDID 读取失败：{}", e));
                        None
                    }
                };
                let cur = d.current.map(|c| (c.width, c.height, c.refresh_hz));
                if cur.is_none() {
                    debug.push("当前模式缺失".into());
                }
                let mut report = dsc::detect(cur, link.as_ref(), edid_dsc, forced);
                // Unknown 时把采集层诊断合并进 state（前端诊断抽屉展示）。
                if let DscState::Unknown { debug: ref mut dd, .. } = report.state {
                    dd.extend(debug);
                }
                d.dsc = report.state.clone();
                if let Some(label) = report.link_label {
                    d.link = Some(DisplayLinkInfo {
                        lane_count: link.map(|l| l.lane_count).unwrap_or(0),
                        rate_per_lane_gbps: link.map(|l| l.rate_gbps).unwrap_or(0.0),
                        link_label: label,
                        bpc: report.bpc,
                        color_format: report.color_format.clone(),
                        available_gbps: report.available_gbps.unwrap_or(0.0),
                        source: "nvapi".into(),
                    });
                }
            }
        }
        #[cfg(not(windows))]
        {
            let _ = displays;
        }
    }

    /// DSC 状态 + 链路详情（诊断抽屉）。
    pub fn dsc_status(&self, target: &ModeTarget) -> Result<(DscState, Option<DisplayLinkInfo>), QrError> {
        let disp = self.resolve(target)?;
        let displays = self.list_displays()?;
        let d = displays
            .into_iter()
            .find(|d| d.gdi_name == disp.gdi_name)
            .ok_or_else(|| QrError::DisplayNotFound(disp.gdi_name.clone()))?;
        Ok((d.dsc, d.link))
    }

    pub fn identify(&self, app: &AppHandle) -> Result<(), QrError> {
        let displays = self.backend.enumerate()?;
        #[cfg(windows)]
        {
            crate::features::quick_resolution::platform::windows::identify::show_identify_overlays(
                app,
                self.backend.as_ref(),
                &displays,
            )
        }
        #[cfg(not(windows))]
        {
            let _ = (app, displays);
            Err(QrError::UnsupportedPlatform)
        }
    }

    fn resolve(&self, target: &ModeTarget) -> Result<ResolvedDisplay, QrError> {
        applier::resolve_target(self.backend.as_ref(), target)
    }

    // ---- 模式 CRUD ----

    pub fn list_modes(&self) -> Vec<DisplayModeEntry> {
        let mut m = self.settings.read().modes.clone();
        m.sort_by_key(|e| e.order);
        m
    }

    /// 新增/更新模式。幂等键 = id（空 id → 新建）。
    pub fn upsert_mode(&self, app: &AppHandle, mut entry: DisplayModeEntry) -> Result<DisplayModeEntry, QrError> {
        if entry.label.trim().is_empty() {
            return Err(QrError::BadRequest("名称不能为空".into()));
        }
        let report = self.validate_mode_internal(&entry)?;
        if !report.errors.is_empty() {
            return Err(QrError::BadRequest(report.errors.join("；")));
        }
        let mut s = self.settings.write();
        if entry.id.is_empty() {
            entry.id = format!("m-{}", now_millis());
            entry.created_at = now_secs();
            entry.order = s.modes.iter().map(|m| m.order).max().map(|m| m + 1).unwrap_or(0);
        }
        // 状态推导：已在系统列表 → Ready/System；否则保持 Draft（待预置）。
        if report.in_system_list {
            entry.state = ModeState::Ready;
            entry.provision_path = Some(ProvisionPath::System);
        } else if entry.provision_path == Some(ProvisionPath::System) {
            // 之前在系统列表而现在不在：标 Stale。
            entry.state = ModeState::Stale;
        }
        match s.modes.iter_mut().find(|m| m.id == entry.id) {
            Some(m) => *m = entry.clone(),
            None => s.modes.push(entry.clone()),
        }
        let saved = s.clone();
        drop(s);
        self.store.save_settings(&saved)?;
        let _ = app.emit("qr://mode-state-changed", ());
        Ok(entry)
    }

    pub fn delete_mode(&self, app: &AppHandle, id: &str) -> Result<(), QrError> {
        let mut s = self.settings.write();
        let before = s.modes.len();
        s.modes.retain(|m| m.id != id);
        let changed = s.modes.len() != before;
        let saved = s.clone();
        drop(s);
        if !changed {
            return Err(QrError::ModeNotFound(id.into()));
        }
        self.store.save_settings(&saved)?;
        let _ = app.emit("qr://mode-state-changed", ());
        Ok(())
    }

    pub fn reorder_modes(&self, app: &AppHandle, ids: Vec<String>) -> Result<(), QrError> {
        let mut s = self.settings.write();
        for (i, id) in ids.iter().enumerate() {
            if let Some(m) = s.modes.iter_mut().find(|m| &m.id == id) {
                m.order = i as u32;
            }
        }
        s.modes.sort_by_key(|m| m.order);
        let saved = s.clone();
        drop(s);
        self.store.save_settings(&saved)?;
        let _ = app.emit("qr://mode-state-changed", ());
        Ok(())
    }

    /// 从系统导入某显示器已有模式（去重：与现有同目标同参数模式比对）。
    pub fn import_system_modes(&self, app: &AppHandle, target: ModeTarget) -> Result<Vec<DisplayModeEntry>, QrError> {
        let disp = self.resolve(&target)?;
        let sys = self.backend.enum_modes(&disp.gdi_name)?;
        let mut created = Vec::new();
        let mut s = self.settings.write();
        let mut order = s.modes.iter().map(|m| m.order).max().map(|m| m + 1).unwrap_or(0);
        for m in sys {
            let exists = s.modes.iter().any(|e| {
                e.width == m.width && e.height == m.height && e.refresh_hz == m.refresh_hz
                    && targets_same_display(&e.target, &target)
            });
            if exists {
                continue;
            }
            let entry = DisplayModeEntry {
                id: format!("m-{}-{}", now_millis(), created.len()),
                label: format!("{}×{} @{}Hz", m.width, m.height, m.refresh_hz),
                width: m.width,
                height: m.height,
                refresh_hz: m.refresh_hz,
                bit_depth: None,
                color_format: None,
                scaling: None,
                target: target.clone(),
                timing_standard: TimingStandardKind::Auto,
                manual_timing: None,
                state: ModeState::Ready,
                provision_path: Some(ProvisionPath::System),
                last_error: None,
                pinned_to_tray: false,
                order,
                hotkey: None,
                skip_confirm: false,
                created_at: now_secs(),
                last_used_at: None,
            };
            order += 1;
            s.modes.push(entry.clone());
            created.push(entry);
        }
        let saved = s.clone();
        drop(s);
        self.store.save_settings(&saved)?;
        let _ = app.emit("qr://mode-state-changed", ());
        Ok(created)
    }

    // ---- 校验 ----

    pub fn validate_mode(&self, entry: &DisplayModeEntry) -> ValidationReport {
        match self.validate_mode_internal(entry) {
            Ok(r) => r,
            Err(e) => ValidationReport {
                ok: false,
                errors: vec![e.to_string()],
                in_system_list: false,
                pixel_clock_khz: 0,
                exceeds_monitor_limit: None,
                feasibility: None,
            },
        }
    }

    fn validate_mode_internal(&self, entry: &DisplayModeEntry) -> Result<ValidationReport, QrError> {
        let mut errors = Vec::new();
        if entry.width < 640 || entry.height < 480 {
            errors.push("分辨率下限 640×480".to_string());
        }
        if entry.width > 16384 || entry.height > 16384 {
            errors.push("分辨率超出合理上限".to_string());
        }
        if !(24..=1000).contains(&entry.refresh_hz) {
            errors.push("刷新率需在 24–1000Hz 整数区间".to_string());
        }
        if entry.timing_standard == TimingStandardKind::Manual && entry.manual_timing.is_none() {
            errors.push("手动 timing 缺少参数".to_string());
        }

        // 系统列表成员检查（目标显示器在线时）。
        let (in_system_list, max_pix) = match self.resolve(&entry.target) {
            Ok(disp) => {
                let in_list = self
                    .backend
                    .enum_modes(&disp.gdi_name)?
                    .iter()
                    .any(|m| m.width == entry.width && m.height == entry.height && m.refresh_hz == entry.refresh_hz);
                let max_pix = self
                    .backend
                    .enumerate()
                    .ok()
                    .and_then(|ds| ds.into_iter().find(|d| d.gdi_name == disp.gdi_name))
                    .and_then(|d| d.max_pixel_clock_khz);
                (in_list, max_pix)
            }
            Err(_) => (false, None),
        };

        // timing 生成 + 像素时钟（native-blanking 继承用原生 timing）。
        let native = self.native_timing_of(&entry.target);
        let standard = to_edid_standard(entry);
        let timing = qr_edid::timing::generate(
            standard,
            entry.width,
            entry.height,
            entry.refresh_hz,
            native.as_ref(),
        );
        let pixel_clock_khz = timing
            .as_ref()
            .map(|t| t.pixel_clock_khz(entry.refresh_hz) as u64)
            .unwrap_or(0);
        if let Err(e) = &timing {
            errors.push(format!("timing 生成失败：{}", e));
        }

        // 显示器上限硬性拦截（display.md §十八-11）。
        let exceeds = max_pix.map(|limit| pixel_clock_khz > limit as u64);
        if exceeds == Some(true) {
            errors.push(format!(
                "超出显示器像素时钟上限 {} kHz",
                max_pix.unwrap_or(0)
            ));
        }

        // M3：带宽可行性（有链路信息时）。
        let feasibility = self.feasibility_of(entry, &timing, pixel_clock_khz);

        Ok(ValidationReport {
            ok: errors.is_empty(),
            errors,
            in_system_list,
            pixel_clock_khz,
            exceeds_monitor_limit: exceeds,
            feasibility,
        })
    }

    /// 目标模式带宽可行性（NVAPI 链路 + EDID DSC 支持 + 用户覆盖）。
    fn feasibility_of(
        &self,
        entry: &DisplayModeEntry,
        timing: &Result<qr_edid::timing::TimingParams, qr_edid::EdidErr>,
        #[allow(unused_variables)] pixel_clock_khz: u64,
    ) -> Option<qr_bandwidth::Feasibility> {
        let t = timing.as_ref().ok()?;
        let disp = self.resolve(&entry.target).ok()?;
        let bpc = entry.bit_depth.unwrap_or(8);
        let cf = entry.color_format.unwrap_or(ColorFormat::RGB).to_bandwidth();
        let bt = qr_bandwidth::Timing {
            h_active: t.h_active,
            v_active: t.v_active,
            h_total: t.h_total(),
            v_total: t.v_total(),
            refresh_hz: entry.refresh_hz,
        };
        #[cfg(windows)]
        {
            use crate::features::quick_resolution::platform::windows::nvapi::NvApi;
            let forced = match self.settings.read().dsc_override {
                DscOverride::Auto => None,
                DscOverride::ForceOn => Some(true),
                DscOverride::ForceOff => Some(false),
            };
            let edid_dsc = self
                .backend
                .read_edid(&disp.key)
                .ok()
                .and_then(|e| crate::features::quick_resolution::platform::windows::dsc::edid_dsc_support(&e))
                .unwrap_or(false);
            let dsc_available = forced.unwrap_or(edid_dsc);
            let api = NvApi::load().ok()?;
            let handle = api.display_handles().into_iter().next()?;
            let link = api.link_info(handle).ok()?;
            let spec = if link.rate_gbps >= 10.0 {
                let mut s = if (link.rate_gbps - 13.5).abs() < 0.1 {
                    qr_bandwidth::LinkSpec::dp_uhbr13_5(link.lane_count)
                } else if (link.rate_gbps - 20.0).abs() < 0.1 {
                    qr_bandwidth::LinkSpec::dp_uhbr20(link.lane_count)
                } else {
                    qr_bandwidth::LinkSpec::dp_uhbr10(link.lane_count)
                };
                s.lanes = link.lane_count;
                s
            } else if link.rate_gbps >= 8.0 {
                let mut s = qr_bandwidth::LinkSpec::dp_hbr3(link.lane_count);
                s.lanes = link.lane_count;
                s
            } else if link.rate_gbps >= 5.0 {
                let mut s = qr_bandwidth::LinkSpec::dp_hbr2(link.lane_count);
                s.lanes = link.lane_count;
                s
            } else {
                let mut s = qr_bandwidth::LinkSpec::dp_hbr(link.lane_count);
                s.lanes = link.lane_count;
                s
            };
            Some(qr_bandwidth::check_feasibility(&bt, bpc, cf, &spec, dsc_available, None))
        }
        #[cfg(not(windows))]
        {
            let _ = (bpc, cf, bt, pixel_clock_khz, disp);
            None
        }
    }

    /// 目标显示器原生 timing（EDID 首条 DTD）。
    fn native_timing_of(&self, target: &ModeTarget) -> Option<qr_edid::timing::TimingParams> {
        let disp = self.resolve(target).ok()?;
        let edid = self.backend.read_edid(&disp.key).ok()?;
        let doc = qr_edid::EdidDoc::parse(&edid).ok()?;
        let info = doc.info();
        qr_edid::parse::native_timing(&info).copied()
    }

    // ---- 快切 ----

    pub async fn apply_by_id(&self, app: &AppHandle, id: &str) -> Result<SwitchResult, QrError> {
        let (mode, confirm, timeout_secs) = {
            let s = self.settings.read();
            let m = s.modes.iter().find(|m| m.id == id).cloned()
                .ok_or_else(|| QrError::ModeNotFound(id.into()))?;
            // skip_confirm 优先于全局 confirm_before_apply。
            let confirm = s.confirm_before_apply && !m.skip_confirm;
            (m, confirm, s.auto_revert_seconds)
        };
        if !mode.state.is_ready() {
            return Err(QrError::ModeNotReady);
        }
        let disp = self.resolve(&mode.target)?;

        // 记录会话原始模式（每显示器一次，restore_on_app_exit 用）。
        if let Some(cur) = self.current_mode_of(&disp.gdi_name) {
            self.session_originals.lock().entry(disp.gdi_name.clone()).or_insert(cur);
            // 托盘「恢复上一个」。
            *self.last_previous.lock() = Some((mode.target.clone(), cur));
        }

        let guard = RollbackGuard::new(self.backend.as_ref())?;
        applier::apply(self.backend.as_ref(), &disp, &mode)?;

        // 状态刷新以系统真相为准。
        self.refresh_states(app);
        // last_used_at
        {
            let mut s = self.settings.write();
            if let Some(m) = s.modes.iter_mut().find(|m| m.id == id) {
                m.last_used_at = Some(now_secs());
            }
            let saved = s.clone();
            drop(s);
            let _ = self.store.save_settings(&saved);
        }

        if !confirm {
            guard.commit();
            return Ok(SwitchResult::Applied);
        }

        // 15s 确认窗口（独立置顶小窗，主窗可能已错位）。
        let (tx, rx) = oneshot::channel::<bool>();
        *self.pending_confirm.lock() = Some(tx);
        open_confirm_window(app, &disp, &mode, timeout_secs);
        let outcome = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs as u64 + 2),
            rx,
        )
        .await;
        close_confirm_window(app);
        *self.pending_confirm.lock() = None;

        match outcome {
            Ok(Ok(true)) => {
                guard.commit();
                Ok(SwitchResult::Applied)
            }
            Ok(Ok(false)) => {
                drop(guard); // RAII 还原
                self.refresh_states(app);
                Ok(SwitchResult::RevertedByUser)
            }
            _ => {
                drop(guard);
                self.refresh_states(app);
                Ok(SwitchResult::RevertedByTimeout)
            }
        }
    }

    /// 托盘「恢复上一个分辨率」。
    pub async fn apply_previous(&self, app: &AppHandle) -> Result<SwitchResult, QrError> {
        let prev = self.last_previous.lock().clone()
            .ok_or_else(|| QrError::BadRequest("没有可恢复的上一个模式".into()))?;
        let entry = DisplayModeEntry {
            id: "__previous__".into(),
            label: "上一个模式".into(),
            width: prev.1.width,
            height: prev.1.height,
            refresh_hz: prev.1.refresh_hz,
            bit_depth: None,
            color_format: None,
            scaling: None,
            target: prev.0,
            timing_standard: TimingStandardKind::Auto,
            manual_timing: None,
            state: ModeState::Ready,
            provision_path: Some(ProvisionPath::System),
            last_error: None,
            pinned_to_tray: false,
            order: 0,
            hotkey: None,
            skip_confirm: false,
            created_at: 0,
            last_used_at: None,
        };
        // 复用确认流：直接内联（不经过 settings 查找）。
        let disp = self.resolve(&entry.target)?;
        let (confirm, timeout_secs) = {
            let s = self.settings.read();
            (s.confirm_before_apply && !entry.skip_confirm, s.auto_revert_seconds)
        };
        let guard = RollbackGuard::new(self.backend.as_ref())?;
        applier::apply(self.backend.as_ref(), &disp, &entry)?;
        self.refresh_states(app);
        if !confirm {
            guard.commit();
            return Ok(SwitchResult::Applied);
        }
        let (tx, rx) = oneshot::channel::<bool>();
        *self.pending_confirm.lock() = Some(tx);
        open_confirm_window(app, &disp, &entry, timeout_secs);
        let outcome = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs as u64 + 2),
            rx,
        )
        .await;
        close_confirm_window(app);
        *self.pending_confirm.lock() = None;
        match outcome {
            Ok(Ok(true)) => {
                guard.commit();
                Ok(SwitchResult::Applied)
            }
            _ => {
                drop(guard);
                self.refresh_states(app);
                Ok(SwitchResult::RevertedByTimeout)
            }
        }
    }

    pub fn confirm_apply(&self) {
        if let Some(tx) = self.pending_confirm.lock().take() {
            let _ = tx.send(true);
        }
    }

    pub fn revert_apply(&self) {
        if let Some(tx) = self.pending_confirm.lock().take() {
            let _ = tx.send(false);
        }
    }

    /// 退出时恢复原始分辨率（restore_on_app_exit）。
    pub fn restore_session_originals(&self) {
        if !self.settings.read().restore_on_app_exit {
            return;
        }
        let originals: Vec<(String, SystemMode)> =
            self.session_originals.lock().iter().map(|(k, v)| (k.clone(), *v)).collect();
        for (gdi, mode) in originals {
            tracing::info!("QR 退出恢复：{} → {}×{} @{}Hz", gdi, mode.width, mode.height, mode.refresh_hz);
            if let Err(e) = self.backend.apply(&gdi, &mode) {
                tracing::warn!("QR 退出恢复失败（{}）：{}", gdi, e);
            }
        }
    }

    // ---- 状态刷新（系统真相 → Stale/Active 标记）----

    /// 以系统模式列表为真相刷新全部模式状态（热插拔/启动自检/切换后调用）。
    pub fn refresh_states(&self, app: &AppHandle) {
        let displays = match self.backend.enumerate() {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!("QR 状态刷新：枚举显示器失败：{}", e);
                return;
            }
        };
        let mut changed = false;
        let mut s = self.settings.write();
        for disp in &displays {
            let sys_modes = match self.backend.enum_modes(&disp.gdi_name) {
                Ok(m) => m,
                Err(_) => continue,
            };
            for m in s.modes.iter_mut() {
                let on_this = match &m.target {
                    ModeTarget::Primary => disp.is_primary,
                    ModeTarget::Index { index } => disp.index == *index,
                    ModeTarget::Key { key } => key == &disp.key,
                };
                if !on_this {
                    continue;
                }
                let in_list = sys_modes.iter().any(|sm| sm.matches(m));
                let is_current = disp.current.map(|c| c.matches(m)).unwrap_or(false);
                let next = if is_current && in_list {
                    ModeState::Active
                } else if in_list {
                    ModeState::Ready
                } else if m.state.is_ready() {
                    ModeState::Stale // 驱动更新/EDID 被重置 → 待 M7 自动重预置
                } else {
                    m.state
                };
                if next != m.state {
                    m.state = next;
                    changed = true;
                }
            }
        }
        let saved = s.clone();
        drop(s);
        if changed {
            let _ = self.store.save_settings(&saved);
            let _ = app.emit("qr://mode-state-changed", ());
        }
    }

    fn current_mode_of(&self, gdi_name: &str) -> Option<SystemMode> {
        self.backend
            .enumerate()
            .ok()?
            .into_iter()
            .find(|d| d.gdi_name == gdi_name)
            .and_then(|d| d.current)
    }

    /// 批量预置（M7）：把 Draft/Validated 模式一次性注入 EDID。
    pub async fn provision(&self, app: &AppHandle, ids: Vec<String>) -> Result<ProvisionReport, QrError> {
        let _lock = self.provision_lock.lock().await; // 全局串行
        let (pending, target) = {
            let s = self.settings.read();
            let pending: Vec<DisplayModeEntry> = if ids.is_empty() {
                s.modes.iter().filter(|m| m.state.is_pending()).cloned().collect()
            } else {
                s.modes.iter().filter(|m| ids.contains(&m.id)).cloned().collect()
            };
            let target = pending.first().map(|m| m.target.clone());
            (pending, target)
        };
        if pending.is_empty() {
            return Err(QrError::BadRequest("没有待预置模式".into()));
        }
        // 实时探测提权能力，不信内存/落盘的 helper_installed 标志
        // （该标志可能因未持久化而过期，导致已具备能力却被误判）。
        // 放行条件二选一：① 主进程自身已是管理员（直写路径）；② 计划任务已注册（helper 转发）。
        let capable = {
            #[cfg(windows)]
            {
                crate::features::quick_resolution::platform::windows::direct_admin::is_elevated()
                    || crate::features::quick_resolution::platform::windows::helper_client::helper_installed()
            }
            #[cfg(not(windows))]
            {
                false
            }
        };
        if !capable {
            return Err(QrError::HelperNotInstalled);
        }
        // 同步内存标志，保持 UI 一致。
        {
            let mut s = self.settings.write();
            if !s.helper_installed {
                s.helper_installed = true;
                let saved = s.clone();
                drop(s);
                let _ = self.store.save_settings(&saved);
            }
        }
        let target = target.ok_or_else(|| QrError::BadRequest("模式无目标显示器".into()))?;
        let disp = self.resolve(&target)?;

        // 标记 provisioning 状态。
        {
            let mut s = self.settings.write();
            for m in s.modes.iter_mut().filter(|m| pending.iter().any(|p| p.id == m.id)) {
                m.state = ModeState::Provisioning;
            }
            let saved = s.clone();
            drop(s);
            let _ = self.store.save_settings(&saved);
            let _ = app.emit("qr://mode-state-changed", ());
        }

        let result = crate::features::quick_resolution::provisioner::provision_batch(
            &self.backend,
            &self.store,
            &disp.key,
            &disp.gdi_name,
            &pending,
        )
        .await;

        // 结果回填。
        let mut s = self.settings.write();
        match &result {
            Ok(report) => {
                for m in s.modes.iter_mut() {
                    if report.succeeded.contains(&m.id) {
                        m.state = ModeState::Ready;
                        m.provision_path = Some(ProvisionPath::Edid);
                    } else if report.failed.contains(&m.id) {
                        m.state = ModeState::Failed;
                        m.last_error = Some(ModeError {
                            code: "ProvisionVerifyFailed".into(),
                            message: "预置验证未通过".into(),
                            at: now_secs(),
                        });
                    }
                }
            }
            Err(e) => {
                for m in s.modes.iter_mut().filter(|m| pending.iter().any(|p| p.id == m.id)) {
                    m.state = ModeState::Failed;
                    m.last_error = Some(ModeError {
                        code: format!("{:?}", e),
                        message: e.to_string(),
                        at: now_secs(),
                    });
                }
            }
        }
        let saved = s.clone();
        drop(s);
        let _ = self.store.save_settings(&saved);
        let _ = app.emit("qr://mode-state-changed", ());
        crate::features::quick_resolution::after_settings_changed(app);
        result
    }

    /// 启动自检（L2）：上次预置未收尾 → 回滚。
    pub fn startup_recovery(&self) -> Option<String> {
        crate::features::quick_resolution::provisioner::startup_recovery_check(&self.store)
    }

    /// 导出诊断包（M9）：zip 到 `%APPDATA%/soundlink/diagnostics/`，返回路径。
    pub fn export_diagnostics(&self) -> Result<String, QrError> {
        let mut dir = self.store.dir().to_path_buf();
        dir.push("diagnostics");
        std::fs::create_dir_all(&dir)?;
        let ts = now_secs();
        let zip_path = dir.join(format!("soundlink-qr-diag-{}.zip", ts));
        let file = std::fs::File::create(&zip_path)?;
        let mut zip = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        // 设置（去敏：不含 license/pairing）。
        let settings_json = serde_json::to_string_pretty(&self.settings())
            .map_err(|e| QrError::Io(e.to_string()))?;
        zip.start_file("quick_resolution.json", opts)
            .map_err(|e| QrError::Io(e.to_string()))?;
        use std::io::Write;
        zip.write_all(settings_json.as_bytes()).map_err(|e| QrError::Io(e.to_string()))?;

        // 能力档案
        let profiles = self.store.load_profiles();
        let profiles_json = serde_json::to_string_pretty(&profiles).map_err(|e| QrError::Io(e.to_string()))?;
        zip.start_file("capability_profiles.json", opts).map_err(|e| QrError::Io(e.to_string()))?;
        zip.write_all(profiles_json.as_bytes()).map_err(|e| QrError::Io(e.to_string()))?;

        // 近 14 天日志
        let logs_dir = crate::logging::log_dir();
        if let Some(ld) = logs_dir {
            if let Ok(entries) = std::fs::read_dir(&ld) {
                for e in entries.flatten() {
                    let p = e.path();
                    let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
                    if name.starts_with("soundlink-") || name.starts_with("helper.log") {
                        if let Ok(content) = std::fs::read(&p) {
                            let _ = zip.start_file(format!("logs/{}", name), opts);
                            let _ = zip.write_all(&content);
                        }
                    }
                }
            }
        }

        // EDID 备份（.bin）
        let backup_dir = self.store.backup_dir();
        if let Ok(entries) = std::fs::read_dir(&backup_dir) {
            for e in entries.flatten() {
                let p = e.path();
                let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
                if name.ends_with(".bin") {
                    if let Ok(content) = std::fs::read(&p) {
                        let _ = zip.start_file(format!("edid/{}", name), opts);
                        let _ = zip.write_all(&content);
                    }
                }
            }
        }

        zip.finish().map_err(|e| QrError::Io(e.to_string()))?;
        Ok(zip_path.to_string_lossy().into_owned())
    }

    // ---- 备份 ----

    pub fn list_backups(&self, target: Option<ModeTarget>) -> Vec<BackupInfo> {
        let key = target.and_then(|t| self.resolve(&t).ok().map(|d| d.key));
        self.store.list_backups(key.as_ref())
    }
}

/// 目标是否指向同一显示器（粗判：同 kind 同参数）。
fn targets_same_display(a: &ModeTarget, b: &ModeTarget) -> bool {
    a == b
}

fn to_edid_standard(entry: &DisplayModeEntry) -> qr_edid::timing::TimingStandard {
    match entry.timing_standard {
        TimingStandardKind::Auto => qr_edid::timing::TimingStandard::Auto,
        TimingStandardKind::CvtRb2 => qr_edid::timing::TimingStandard::CvtRb2,
        TimingStandardKind::CvtRb3 => qr_edid::timing::TimingStandard::CvtRb3,
        TimingStandardKind::Manual => {
            let m = entry.manual_timing.unwrap_or(ManualTiming {
                h_front: 8,
                h_sync: 32,
                h_back: 40,
                v_front: 3,
                v_sync: 5,
                v_back: 20,
                h_sync_pol: false,
                v_sync_pol: true,
            });
            qr_edid::timing::TimingStandard::Manual(qr_edid::timing::TimingParams {
                h_active: entry.width,
                v_active: entry.height,
                h_front: m.h_front,
                h_sync: m.h_sync,
                h_back: m.h_back,
                v_front: m.v_front,
                v_sync: m.v_sync,
                v_back: m.v_back,
                h_sync_pol: m.h_sync_pol,
                v_sync_pol: m.v_sync_pol,
                interlaced: false,
            })
        }
    }
}

/// 打开 15s 确认窗（独立置顶小窗）。
fn open_confirm_window(app: &AppHandle, disp: &ResolvedDisplay, mode: &DisplayModeEntry, timeout_secs: u32) {
    close_confirm_window(app);
    let url = format!(
        "index.html?view=qr-confirm&mode={}&timeout={}&display={}",
        urlencoding_simple(&mode.brief()),
        timeout_secs,
        disp.index
    );
    // 定位到目标显示器中央。
    let (mut x, mut y) = (100.0f64, 100.0f64);
    #[cfg(windows)]
    {
        if let Ok((mx, my, mw, mh)) =
            crate::features::quick_resolution::platform::windows::gdi::monitor_rect(&disp.gdi_name)
        {
            x = mx as f64 + mw as f64 / 2.0 - CONFIRM_W / 2.0;
            y = my as f64 + mh as f64 / 2.0 - CONFIRM_H / 2.0;
        }
    }
    match WebviewWindowBuilder::new(app, "qr-confirm", WebviewUrl::App(url.into()))
        .title("确认分辨率切换")
        .decorations(true)
        .always_on_top(true)
        .resizable(false)
        .skip_taskbar(true)
        .inner_size(CONFIRM_W, CONFIRM_H)
        .position(x, y)
        .build()
    {
        Ok(_) => {}
        Err(e) => tracing::warn!("QR 确认窗创建失败：{}（超时将自动回滚）", e),
    }
}

fn close_confirm_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("qr-confirm") {
        let _ = w.close();
    }
}

/// 极简 URL 编码（仅需处理空格/×/@等非 ASCII 与保留字符）。
fn urlencoding_simple(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urlencoding_ascii_passthrough() {
        assert_eq!(urlencoding_simple("abc-123_"), "abc-123_");
    }

    #[test]
    fn urlencoding_encodes_non_ascii() {
        let e = urlencoding_simple("1920×1440 @480Hz");
        assert!(!e.contains('×'));
        assert!(!e.contains(' '));
        assert!(e.contains("%40")); // '@'
    }

    #[test]
    fn settings_clamp() {
        // 数值夹取逻辑（不依赖后端，构造 settings 直接校验规则）。
        let mut s = QuickResolutionSettings::default();
        s.auto_revert_seconds = 999;
        s.max_tray_items = 0;
        // clamp 逻辑与 save_settings 内一致：
        s.auto_revert_seconds = s.auto_revert_seconds.clamp(5, 60);
        s.max_tray_items = s.max_tray_items.clamp(1, 16);
        assert_eq!(s.auto_revert_seconds, 60);
        assert_eq!(s.max_tray_items, 1);
    }
}
