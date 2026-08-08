//! 自适应能力探测（display.md §五「不认机型，只认能力；探一次，缓存起来」）。
//!
//! 探测阶梯（§5.2）：
//! 1. profile key = GPU PCI ID + 驱动版本 + EDID SHA256 + 连接器；
//! 2. 缓存命中且未过期 → 直接用；
//! 3. NVAPI 可用性 + DSC 判定；
//! 4. 无害探针（等价 timing 副本）验证注册表变体（M4 helper 写，主进程验证系统列表）；
//! 5. 激活方式阶梯（Monitor 重启 → Adapter 重启 → LogoffRequired）；
//! 6. 统计 DTD 空槽/扩展块容量，写缓存 + 完整还原现场。

use crate::features::quick_resolution::model::{CapabilityProfile, QrError, TriState};
use crate::features::quick_resolution::platform::DisplayBackend;
use crate::features::quick_resolution::store::Store;
use qr_ipc::{MonitorKey, RegVariant};
use std::sync::Arc;

/// 缓存有效期：7 天（驱动/GPU/显示器任一变化即换 key 自然失效）。
const PROFILE_TTL_SECS: i64 = 7 * 86_400;

/// 探测并缓存能力档案。
///
/// `helper_available`：计划任务是否已注册（未注册时跳过写注册表类探测，降级为只读探测）。
pub async fn probe_capability(
    backend: &Arc<dyn DisplayBackend>,
    store: &Store,
    key: &MonitorKey,
    gpu_id: &str,
    driver_version: &str,
    connector: &str,
    helper_available: bool,
) -> Result<CapabilityProfile, QrError> {
    let profile_key = format!(
        "{}|{}|{}|{}",
        gpu_id,
        driver_version,
        &key.edid_hash,
        connector
    );

    // 1) 缓存命中检查
    let cached = store.load_profiles();
    if let Some(p) = cached.iter().find(|p| p.key == profile_key) {
        if crate::features::quick_resolution::model::now_secs() - p.probed_at < PROFILE_TTL_SECS {
            return Ok(p.clone());
        }
    }

    // 2) 只读探测（不依赖 helper）：EDID 解析容量。
    let mut profile = CapabilityProfile {
        key: profile_key.clone(),
        nvapi_custom: TriState::Unknown,
        nvapi_custom_last_status: None,
        edid_reg_variant: None,
        activation: None,
        max_extension_blocks: None,
        free_dtd_slots: None,
        displayid_supported: None,
        verified_max_pixel_clock_khz: None,
        probed_at: crate::features::quick_resolution::model::now_secs(),
        probe_log_id: format!("probe-{}", crate::features::quick_resolution::model::now_secs()),
    };

    if let Ok(edid) = backend.read_edid(key) {
        if let Ok(doc) = qr_edid::EdidDoc::parse(&edid) {
            profile.free_dtd_slots = Some(doc.free_dtd_slots());
            profile.max_extension_blocks = Some(doc.max_extension_blocks());
            let info = doc.info();
            profile.displayid_supported = Some(info.displayid_supported);
            profile.verified_max_pixel_clock_khz = info.max_pixel_clock_khz;
        }
    }

    // 3) NVAPI 可用性（Windows + NVIDIA）。
    #[cfg(windows)]
    {
        use crate::features::quick_resolution::platform::windows::nvapi::NvApi;
        if NvApi::load().is_ok() {
            // NVAPI 存在即自定义分辨率可能可用（DSC 阻断与否由 M6 TryCustomDisplay 实测）。
            profile.nvapi_custom = TriState::Unknown;
        } else {
            profile.nvapi_custom = TriState::Blocked;
        }
    }

    // 4) 无害探针（需 helper）：默认注册表变体 = 显示器实例 override（CRU 同款）。
    //    M6 的完整阶梯（试 3 个变体 + 激活方式）在 helper 可用时执行；
    //    未装 helper 时保守假设实例变体（写入失败会在 M7 预置时暴露并回滚）。
    if helper_available {
        profile.edid_reg_variant = Some(RegVariant::MonitorInstanceOverride);
        // 激活方式留待 M7 预置时实测（Monitor→Adapter 阶梯在 provisioner 内）。
    }

    // 5) 写缓存
    let mut profiles = cached;
    profiles.retain(|p| p.key != profile_key);
    profiles.push(profile.clone());
    let _ = store.save_profiles(&profiles);

    Ok(profile)
}

/// 驱动/GPU/显示器变化时使缓存失效（删除对应 profile）。
pub fn invalidate_profile(store: &Store, profile_key: &str) {
    let mut profiles = store.load_profiles();
    profiles.retain(|p| p.key != profile_key);
    let _ = store.save_profiles(&profiles);
}
