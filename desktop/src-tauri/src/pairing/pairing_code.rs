//! 配对码生成与校验。8 位数字，有效期 120s，尝试 5 次后失效。
//! 对齐 `docs/First/11-implementation-spec.md` §1 / §5。

use crate::constants::{
    PAIRING_CODE_DIGITS, PAIRING_CODE_MAX_ATTEMPTS, PAIRING_CODE_TTL_SECS,
    PAIRING_LOCK_DURATION_SECS,
};
use parking_lot::Mutex;
use rand::Rng;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct PairingCode {
    pub code: String,
    pub created_at: Instant,
    pub attempts: u32,
    /// D4：超限锁定到期时刻。None 表示未锁定；Some(t) 表示在 t 之前不可重试。
    pub locked_until: Option<Instant>,
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
            locked_until: None,
        }
    }

    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() > Duration::from_secs(PAIRING_CODE_TTL_SECS)
    }

    /// 是否处于锁定状态。D4：兼顾尝试次数与锁定时长，锁定时长过期后视为未锁定。
    pub fn is_locked(&self) -> bool {
        if self.attempts < PAIRING_CODE_MAX_ATTEMPTS {
            return false;
        }
        match self.locked_until {
            None => true, // 旧逻辑：仅次数超限即锁定
            Some(t) => Instant::now() < t,
        }
    }

    /// D4：剩余锁定秒数（0 表示已解锁或未锁定）。
    pub fn remaining_lock_secs(&self) -> u64 {
        match self.locked_until {
            Some(t) => {
                let now = Instant::now();
                if now >= t {
                    0
                } else {
                    t.duration_since(now).as_secs()
                }
            }
            None => 0,
        }
    }

    /// D4：剩余可尝试次数（已锁定时返回 0）。
    pub fn remaining_attempts(&self) -> u32 {
        self.attempts.min(PAIRING_CODE_MAX_ATTEMPTS)
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
    fixed_code: Mutex<Option<String>>,
    /// DEBUG 模式：[`issue`](Self::issue) 固定返回 `12345678`，便于开发期固定码连接。
    debug: bool,
}

impl Default for PairingCodeManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PairingCodeManager {
    pub fn new() -> Self {
        Self::with_debug(false)
    }

    /// `debug = true` 时 [`issue`](Self::issue) 返回固定码 `12345678`。
    pub fn with_debug(debug: bool) -> Self {
        Self {
            current: Mutex::new(None),
            fixed_code: Mutex::new(None),
            debug,
        }
    }

    pub fn set_fixed_code(&self, code: Option<String>) -> Result<(), String> {
        if let Some(code) = code.as_deref() {
            validate_pairing_code(code)?;
        }
        *self.fixed_code.lock() = code;
        *self.current.lock() = None;
        Ok(())
    }

    pub fn fixed_code(&self) -> Option<String> {
        self.fixed_code.lock().clone()
    }

    /// 生成新配对码（覆盖旧的）。
    pub fn issue(&self) -> String {
        if let Some(code) = self.fixed_code.lock().clone() {
            let pc = PairingCode {
                code,
                created_at: Instant::now(),
                attempts: 0,
                locked_until: None,
            };
            let code = pc.code.clone();
            *self.current.lock() = Some(pc);
            return code;
        }
        if self.debug {
            let pc = PairingCode {
                code: "12345678".into(),
                created_at: Instant::now(),
                attempts: 0,
                locked_until: None,
            };
            let code = pc.code.clone();
            *self.current.lock() = Some(pc);
            return code;
        }
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
            // D4：设置锁定到期时刻，达到 MAX_ATTEMPTS 后进入锁定窗口。
            pc.locked_until =
                Some(Instant::now() + Duration::from_secs(PAIRING_LOCK_DURATION_SECS));
            PairingCodeState::Locked
        } else {
            PairingCodeState::Wrong
        }
    }

    /// D4：当前锁定状态快照。返回 `(is_locked, remaining_secs, remaining_attempts)`。
    /// `remaining_attempts` 表示已用尝试次数（达到上限时返回 MAX_ATTEMPTS）。
    pub fn lock_status(&self) -> (bool, u64, u32) {
        let guard = self.current.lock();
        match guard.as_ref() {
            Some(pc) => (pc.is_locked(), pc.remaining_lock_secs(), pc.attempts),
            None => (false, 0, 0),
        }
    }
}

fn validate_pairing_code(code: &str) -> Result<(), String> {
    if code.len() != PAIRING_CODE_DIGITS || !code.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("配对码必须是 {} 位数字", PAIRING_CODE_DIGITS));
    }
    Ok(())
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

    #[test]
    fn debug_mode_issues_fixed_code() {
        let m = PairingCodeManager::with_debug(true);
        assert_eq!(m.issue(), "12345678");
        // 固定码同样可被校验通过。
        assert_eq!(m.verify("12345678"), PairingCodeState::Ok);
    }

    #[test]
    fn user_fixed_code_overrides_random_and_debug() {
        let m = PairingCodeManager::with_debug(true);
        m.set_fixed_code(Some("87654321".into())).unwrap();
        assert_eq!(m.fixed_code().as_deref(), Some("87654321"));
        assert_eq!(m.issue(), "87654321");
        assert_eq!(m.verify("87654321"), PairingCodeState::Ok);
    }

    #[test]
    fn user_fixed_code_must_be_eight_digits() {
        let m = PairingCodeManager::new();
        assert!(m.set_fixed_code(Some("1234".into())).is_err());
        assert!(m.set_fixed_code(Some("abcdefgh".into())).is_err());
    }

    /// D4：超限锁定后 lock_status 返回锁定状态与剩余秒数。
    #[test]
    fn lock_status_after_max_attempts() {
        let m = PairingCodeManager::new();
        let _ = m.issue();
        // 未锁定前：is_locked=false, remaining_secs=0
        let (locked, _, attempts) = m.lock_status();
        assert!(!locked);
        assert_eq!(attempts, 0);
        for _ in 0..PAIRING_CODE_MAX_ATTEMPTS {
            let _ = m.verify("00000000");
        }
        let (locked, remaining, attempts) = m.lock_status();
        assert!(locked);
        assert_eq!(attempts, PAIRING_CODE_MAX_ATTEMPTS);
        // 锁定窗口 60s
        assert!(
            remaining <= PAIRING_LOCK_DURATION_SECS && remaining > PAIRING_LOCK_DURATION_SECS - 5
        );
    }
}
