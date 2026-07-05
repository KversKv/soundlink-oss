//! 配对与密钥协商。第一版：X25519 + HKDF + HMAC（对齐 spec §5）。
//!
//! - 配对码派生 `pairing_secret`。
//! - X25519 会话密钥协商 → `session_master` → `audio_key` / `control_key`。
//! - HMAC 证明（防中间人，控制面校验）。
//!
//! 升级 SPAKE2/SRP 见 `docs/First/05-pairing-security.md`。

use crate::constants::{
    AEAD_KEY_LEN, AUDIO_KEY_INFO, CONTROL_KEY_INFO, PAIRING_SALT, PROTOCOL_VERSION, SESSION_INFO,
};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use rand_core::OsRng;
use sha2::Sha256;
use x25519_dalek::{EphemeralSecret, PublicKey};

type HmacSha256 = Hmac<Sha256>;

/// 配对码 → pairing_secret。
/// `pairing_secret = HKDF-SHA256(ikm=pairing_code, salt="soundlink-pair-v1", info=receiver_device_id, len=32)`
pub fn derive_pairing_secret(pairing_code: &str, receiver_device_id: &str) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(Some(PAIRING_SALT), pairing_code.as_bytes());
    let mut out = [0u8; 32];
    hk.expand(receiver_device_id.as_bytes(), &mut out)
        .expect("32 字节展开不会失败");
    out
}

/// Sender 侧证明：`proof = HMAC-SHA256(pairing_secret, sender_pub ‖ receiver_device_id ‖ protocol_version)`。
pub fn sender_proof(
    pairing_secret: &[u8; 32],
    sender_pub: &PublicKey,
    receiver_device_id: &str,
) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(pairing_secret).unwrap();
    mac.update(sender_pub.as_bytes());
    mac.update(receiver_device_id.as_bytes());
    mac.update(&[PROTOCOL_VERSION]);
    let mut out = [0u8; 32];
    out.copy_from_slice(&mac.finalize().into_bytes());
    out
}

/// Receiver 侧回证：`proof' = HMAC-SHA256(pairing_secret, receiver_pub ‖ sender_pub ‖ receiver_device_id)`。
pub fn receiver_proof(
    pairing_secret: &[u8; 32],
    receiver_pub: &PublicKey,
    sender_pub: &PublicKey,
    receiver_device_id: &str,
) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(pairing_secret).unwrap();
    mac.update(receiver_pub.as_bytes());
    mac.update(sender_pub.as_bytes());
    mac.update(receiver_device_id.as_bytes());
    let mut out = [0u8; 32];
    out.copy_from_slice(&mac.finalize().into_bytes());
    out
}

/// 校验 Sender 证明。
pub fn verify_sender_proof(
    pairing_secret: &[u8; 32],
    sender_pub: &PublicKey,
    receiver_device_id: &str,
    proof: &[u8; 32],
) -> bool {
    let expected = sender_proof(pairing_secret, sender_pub, receiver_device_id);
    constant_time_eq(&expected, proof)
}

/// 校验 Receiver 回证。
pub fn verify_receiver_proof(
    pairing_secret: &[u8; 32],
    receiver_pub: &PublicKey,
    sender_pub: &PublicKey,
    receiver_device_id: &str,
    proof: &[u8; 32],
) -> bool {
    let expected = receiver_proof(pairing_secret, receiver_pub, sender_pub, receiver_device_id);
    constant_time_eq(&expected, proof)
}

fn constant_time_eq(a: &[u8; 32], b: &[u8; 32]) -> bool {
    use subtle::ConstantTimeEq;
    a.ct_eq(b).into()
}

/// 会话密钥集合。
#[derive(Debug, Clone)]
pub struct SessionKeys {
    pub audio_key: [u8; AEAD_KEY_LEN],
    pub control_key: [u8; AEAD_KEY_LEN],
}

/// 由 X25519 共享秘密 + pairing_secret 派生会话密钥。
///
/// - `shared = X25519(own_priv, peer_pub)`
/// - `session_master = HKDF(ikm=shared, salt=pairing_secret, info="soundlink-session-v1", len=32)`
/// - `audio_key = HKDF(ikm=session_master, salt="", info="audio", len=32)`
/// - `control_key = HKDF(ikm=session_master, salt="", info="control", len=32)`
pub fn derive_session_keys(shared_secret: &[u8; 32], pairing_secret: &[u8; 32]) -> SessionKeys {
    let hk = Hkdf::<Sha256>::new(Some(pairing_secret), shared_secret);
    let mut session_master = [0u8; 32];
    hk.expand(SESSION_INFO, &mut session_master)
        .expect("32 字节展开不会失败");

    let hk_audio = Hkdf::<Sha256>::new(Some(&[]), &session_master);
    let mut audio_key = [0u8; AEAD_KEY_LEN];
    hk_audio
        .expand(AUDIO_KEY_INFO, &mut audio_key)
        .expect("32 字节展开不会失败");

    let hk_ctrl = Hkdf::<Sha256>::new(Some(&[]), &session_master);
    let mut control_key = [0u8; AEAD_KEY_LEN];
    hk_ctrl
        .expand(CONTROL_KEY_INFO, &mut control_key)
        .expect("32 字节展开不会失败");

    SessionKeys {
        audio_key,
        control_key,
    }
}

/// 一次性 X25519 密钥对（会话用）。
pub struct EphemeralKeyPair {
    pub secret: EphemeralSecret,
    pub public: PublicKey,
}

impl EphemeralKeyPair {
    pub fn generate() -> Self {
        let secret = EphemeralSecret::random_from_rng(OsRng);
        let public = PublicKey::from(&secret);
        Self { secret, public }
    }
}

/// 计算共享秘密（消耗 own_priv：ephemeral secret 单次使用）。
pub fn diffie_hellman(own: EphemeralSecret, peer_pub: &PublicKey) -> [u8; 32] {
    own.diffie_hellman(peer_pub).to_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairing_secret_is_deterministic() {
        let a = derive_pairing_secret("12345678", "pc-aaaa");
        let b = derive_pairing_secret("12345678", "pc-aaaa");
        assert_eq!(a, b);
        // 不同 device_id 派生不同
        let c = derive_pairing_secret("12345678", "pc-bbbb");
        assert_ne!(a, c);
    }

    #[test]
    fn session_keys_match_on_both_sides() {
        let recv = EphemeralKeyPair::generate();
        let send = EphemeralKeyPair::generate();

        let shared_recv = diffie_hellman(recv.secret, &send.public);
        let shared_send = diffie_hellman(send.secret, &recv.public);
        assert_eq!(shared_recv, shared_send);

        let pairing_secret = derive_pairing_secret("87654321", "pc-xyz");
        let keys_recv = derive_session_keys(&shared_recv, &pairing_secret);
        let keys_send = derive_session_keys(&shared_send, &pairing_secret);
        assert_eq!(keys_recv.audio_key, keys_send.audio_key);
        assert_eq!(keys_recv.control_key, keys_send.control_key);
    }

    #[test]
    fn proof_roundtrip() {
        let recv = EphemeralKeyPair::generate();
        let send = EphemeralKeyPair::generate();
        let pairing_secret = derive_pairing_secret("11223344", "pc-r");
        let sp = sender_proof(&pairing_secret, &send.public, "pc-r");
        assert!(verify_sender_proof(
            &pairing_secret,
            &send.public,
            "pc-r",
            &sp
        ));
        // 篡改后校验失败
        let mut bad = sp;
        bad[0] ^= 0xFF;
        assert!(!verify_sender_proof(
            &pairing_secret,
            &send.public,
            "pc-r",
            &bad
        ));

        let rp = receiver_proof(&pairing_secret, &recv.public, &send.public, "pc-r");
        assert!(verify_receiver_proof(
            &pairing_secret,
            &recv.public,
            &send.public,
            "pc-r",
            &rp
        ));
    }
}
