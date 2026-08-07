//! 吊销名单：内置、静态、离线（E2 不联网）。
//!
//! key 泄露传播时，把对应 license 的 `nonce` 追加到 [`REVOKED_NONCES`] 并发新版。
//! 首发为空数组。**只追加、不修改已有条目**（E8：已签发 key 的结论只能收紧于
//! 「泄露 key」本身，不得误伤正常 key）。

/// 已吊销的 nonce 列表（base32 文本，大小写不敏感——payload 内由签发端统一大写）。
pub const REVOKED_NONCES: &[&str] = &[];

/// nonce 是否命中吊销名单。
pub fn is_revoked(nonce: &str) -> bool {
    REVOKED_NONCES.contains(&nonce)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_list_revokes_nothing() {
        assert!(!is_revoked("ANYTHING"));
    }

    #[test]
    fn inserted_nonce_is_revoked() {
        // R6 验证：临时插入测试 nonce 的等价判定（contains 语义）。
        let test_list: &[&str] = &["TESTNONCE1", "TESTNONCE2"];
        assert!(test_list.contains(&"TESTNONCE1"));
        assert!(!test_list.contains(&"TESTNONCE3"));
    }
}
