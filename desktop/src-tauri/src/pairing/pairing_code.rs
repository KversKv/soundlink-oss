//! 配对码生成与校验。8 位数字，有效期 120s，尝试 5 次后失效。
//! 对齐 `docs/First/11-implementation-spec.md` §1 / §5。

use crate::constants::{PAIRING_CODE_DIGITS, PAIRING_CODE_MAX_ATTEMPTS, PAIRING_CODE_TTL_SECS};
use parking_lot::Mutex;
use rand::Rng;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct PairingCode {
    pub code: String,
    pub created_at: Instant,
    pub attempts: u32,
}

impl PairingCode {
    pub fn generate() -> Self {
        let mut rng = rand::thread_rng();
        let n: u32 = rng.gen_range(0..10u32.pow(PAIRING_CODE_DIGITS as u32));
        let code = format!("{:0width$}", n, width = PAIRING_CODE_DIGITS);
        Self {
            code,
            created_at: Instant::now(),
            attempts: 0,
        }
    }

    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() > Duration::from_secs(PAIRING_CODE_TTL_SECS)
    }

    pub fn is_locked(&self) -> bool {
        self.attempts >= PAIRING_CODE_MAX_ATTEMPTS
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairingCodeState {
    /// 校验通过。
    Ok,
    /// 配对码错误（未超限）。
    Wrong,
    /// 已过期。
    Expired,
    /// 尝试次数超限，锁定。
    Locked,
}

/// 配对码管理器：生成、校验、过期/锁定。
pub struct PairingCodeManager {
    current: Mutex<Option<PairingCode>>,
}

impl Default for PairingCodeManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PairingCodeManager {
    pub fn new() -> Self {
        Self {
            current: Mutex::new(None),
        }
    }

    /// 生成新配对码（覆盖旧的）。
    pub fn issue(&self) -> String {
        let pc = PairingCode::generate();
        let code = pc.code.clone();
        *self.current.lock() = Some(pc);
        code
    }

    /// 当前配对码（若存在且未过期）。
    pub fn current(&self) -> Option<String> {
        self.current
            .lock()
            .as_ref()
            .filter(|c| !c.is_expired() && !c.is_locked())
            .map(|c| c.code.clone())
    }

    /// 校验配对码。校验失败会增加尝试计数。
    pub fn verify(&self, input: &str) -> PairingCodeState {
        let mut guard = self.current.lock();
        let Some(pc) = guard.as_mut() else {
            return PairingCodeState::Expired;
        };
        if pc.is_locked() {
            return PairingCodeState::Locked;
        }
        if pc.is_expired() {
            return PairingCodeState::Expired;
        }
        if pc.code == input {
            // 校验通过，作废当前码（一次性）。
            *guard = None;
            return PairingCodeState::Ok;
        }
        pc.attempts += 1;
        if pc.attempts >= PAIRING_CODE_MAX_ATTEMPTS {
            PairingCodeState::Locked
        } else {
            PairingCodeState::Wrong
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issue_and_verify_ok() {
        let m = PairingCodeManager::new();
        let c = m.issue();
        assert_eq!(c.len(), PAIRING_CODE_DIGITS);
        assert_eq!(m.verify(&c), PairingCodeState::Ok);
        // 一次性：再次校验同码应过期（已作废）
        assert_eq!(m.verify(&c), PairingCodeState::Expired);
    }

    #[test]
    fn wrong_attempts_then_lock() {
        let m = PairingCodeManager::new();
        let _ = m.issue();
        for _ in 0..(PAIRING_CODE_MAX_ATTEMPTS - 1) {
            assert_eq!(m.verify("00000000"), PairingCodeState::Wrong);
        }
        assert_eq!(m.verify("00000000"), PairingCodeState::Locked);
    }
}
