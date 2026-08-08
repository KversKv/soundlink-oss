//! 回滚保险（display.md §7.4）。
//!
//! - [`RollbackGuard`]：切换前快照，未 `commit` 即 Drop 时自动还原（RAII）。
//! - L2 启动自检在 M7 随预置落地（`pending_recovery.json` 由 store 提供）。

use crate::features::quick_resolution::model::QrError;
use crate::features::quick_resolution::platform::{DisplayBackend, DisplaySnapshot};

/// RAII 回滚守卫。
pub struct RollbackGuard<'a> {
    backend: &'a dyn DisplayBackend,
    snap: DisplaySnapshot,
    committed: bool,
}

impl<'a> RollbackGuard<'a> {
    pub fn new(backend: &'a dyn DisplayBackend) -> Result<Self, QrError> {
        let snap = backend.snapshot()?;
        Ok(Self { backend, snap, committed: false })
    }

    /// 确认生效：放弃回滚权。
    pub fn commit(mut self) {
        self.committed = true;
    }
}

impl Drop for RollbackGuard<'_> {
    fn drop(&mut self) {
        if !self.committed {
            tracing::info!("QR 回滚守卫触发：还原显示拓扑快照");
            if let Err(e) = self.backend.restore(&self.snap) {
                tracing::error!("QR 快照还原失败：{}", e);
            }
        }
    }
}
