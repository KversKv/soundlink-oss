//! 配对与信任。
pub mod key_exchange;
pub mod pairing_code;
pub mod trust_store;

pub use key_exchange::{
    derive_pairing_secret, derive_session_keys, diffie_hellman, receiver_proof, sender_proof,
    verify_receiver_proof, verify_sender_proof, EphemeralKeyPair, SessionKeys,
};
pub use pairing_code::{PairingCodeManager, PairingCodeState};
pub use trust_store::{TrustStore, TrustedDevice};
