//! License 令牌：格式解析、Ed25519 验签、payload 校验。
//!
//! 格式：`SLPRO-<base32(payload_json)>-<base32(ed25519_sig)>`
//! （校验时忽略 `-` 与空白、统一大写；签名对象为 base32 解码后的 payload 原始字节，
//! 因此客户端永远不需要重新序列化 JSON，天然规避跨语言 canonical 差异。）
//!
//! 跨版本兼容硬约束（MON-01 §4.2 / E8：已签发的 key 永久有效）：
//! - C1：[`PUBKEYS_VENDOR_B64`] 只增不减，验签任一命中即通过；
//! - C2：payload 版本判定用 `v <= LICENSE_FORMAT_MAX`，禁止 `v == 1`；
//! - C4：[`SKU_WHITELIST`] 只增不减，`"desktop-pro"` 永久在列；
//! - C5：新增 payload 字段一律 `#[serde(default)]` 且缺失时取宽松默认。

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

/// 当前支持的 license 格式版本上限（payload.v ≤ 此值即接受）。
pub const LICENSE_FORMAT_MAX: u8 = 1;

/// SKU 白名单（C4：只增不减）。
pub const SKU_WHITELIST: &[&str] = &["desktop-pro"];

/// vendor 验签公钥（base64，Ed25519 32 字节）。
///
/// C1：一经发布永不删除；轮换密钥时新公钥**追加**到数组末尾。
/// 签发私钥写死于私仓 `pro/license/vendor_sk.hex`（唯一权威来源，
/// 见私仓 `license/README.md`），与本数组第一项一一对应。
pub const PUBKEYS_VENDOR_B64: &[&str] = &[
    // MON-01 R2：首发 vendor 公钥（私仓 pro/license/vendor_sk.hex 对应公钥；
    // 曾误填 2026-08-06 临时密钥对的公钥，因该对私钥未留存且从未随发布版流出，已更正）。
    // C1：永不删除；轮换时新公钥追加到末尾。
    "wKpxUUe0XZsacDcV2sAKXU9K7wGCiQxUk369M6PJvqU=",
];

/// 拒绝原因 → 映射到 `LicenseState`（任何非 Active 一律等价免费版，E1）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reject {
    /// 格式/签名/内容无效，附带人类可读原因。
    Invalid(String),
    /// `exp` 非空且已过期。
    Expired,
    /// nonce 命中内置吊销名单。
    Revoked,
    /// `bind=fingerprint` 但与本机指纹不符。
    DeviceMismatch,
}

/// License payload（签名 JSON）。
///
/// C5：后续新增字段必须 `#[serde(default)]` 且缺失时取宽松默认；
/// 未知多余字段被 serde 默认忽略（旧 key 在新版本下仍可通过，R10）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LicensePayload {
    /// 格式版本，当前 1。
    pub v: u8,
    /// 产品标识，如 "desktop-pro"。
    pub sku: String,
    /// 签发时间（Unix 秒）。
    pub iat: u64,
    /// 过期时间；买断留空表示永久（买断 key 永不写 exp）。
    #[serde(default)]
    pub exp: Option<u64>,
    /// 买家标识：设备指纹（bind=fingerprint）或订单号哈希（bind=order）。
    pub sub: String,
    /// 绑定方式："fingerprint" / "order"。
    pub bind: String,
    /// 允许设备数，默认 3。
    #[serde(default = "default_seats")]
    pub seats: u8,
    /// 8 字节随机 base32，用于吊销与泄露溯源。
    pub nonce: String,
}

fn default_seats() -> u8 {
    3
}

/// 解码全部 vendor 公钥。损坏条目跳过（宁可少一把钥匙也不 panic，E1）。
pub fn vendor_keys() -> Vec<VerifyingKey> {
    PUBKEYS_VENDOR_B64
        .iter()
        .filter_map(|b64| {
            use base64::{engine::general_purpose::STANDARD, Engine};
            let bytes = STANDARD.decode(b64).ok()?;
            let arr: [u8; 32] = bytes.try_into().ok()?;
            VerifyingKey::from_bytes(&arr).ok()
        })
        .collect()
}

/// 校验 license 文本。成功返回 payload；失败返回拒绝原因。
///
/// `fingerprints`：本机指纹候选集（R9，任一命中即通过）。
/// `max_version`：可注入的格式版本上限（生产用 [`LICENSE_FORMAT_MAX`]，测试可放宽）。
pub fn validate_token(
    raw: &str,
    now: u64,
    fingerprints: &[String],
    keys: &[VerifyingKey],
    revoked: &[&str],
    max_version: u8,
) -> Result<LicensePayload, Reject> {
    // 规范化：去空白、统一大写（用户复制粘贴可能带换行/空格）。
    let cleaned: String = raw
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_uppercase();
    let parts: Vec<&str> = cleaned.split('-').filter(|s| !s.is_empty()).collect();
    if parts.len() < 3 || parts[0] != "SLPRO" {
        return Err(Reject::Invalid("授权码格式错误（前缀应为 SLPRO）".into()));
    }
    // 宽容拆分：首段前缀、末段签名、中间全部并入 payload（容忍分组短横线）。
    let sig_b32 = parts[parts.len() - 1];
    let payload_b32: String = parts[1..parts.len() - 1].concat();

    let payload_bytes = base32_decode(&payload_b32)
        .map_err(|e| Reject::Invalid(format!("payload 解码失败：{}", e)))?;
    let sig_bytes =
        base32_decode(sig_b32).map_err(|e| Reject::Invalid(format!("签名解码失败：{}", e)))?;
    let sig = Signature::from_slice(&sig_bytes)
        .map_err(|_| Reject::Invalid("签名格式错误".to_string()))?;

    // C1：任一公钥命中即通过。
    let verified = keys.iter().any(|k| k.verify(&payload_bytes, &sig).is_ok());
    if !verified {
        return Err(Reject::Invalid("签名校验失败".into()));
    }

    let payload: LicensePayload = serde_json::from_slice(&payload_bytes)
        .map_err(|e| Reject::Invalid(format!("payload 解析失败：{}", e)))?;

    // C2：向后兼容旧格式（v ≤ 上限即接受；v=0 非法）。
    if payload.v == 0 || payload.v > max_version {
        return Err(Reject::Invalid("授权码格式版本不受支持".into()));
    }
    // C4：SKU 白名单。
    if !SKU_WHITELIST.contains(&payload.sku.as_str()) {
        return Err(Reject::Invalid("授权码产品标识（SKU）不匹配".into()));
    }
    if payload.sub.is_empty() || payload.nonce.is_empty() {
        return Err(Reject::Invalid("授权码缺少必要字段".into()));
    }
    // 买断 key exp 为空；只有限时授权才读系统时钟。
    if let Some(exp) = payload.exp {
        if now > exp {
            return Err(Reject::Expired);
        }
    }
    if revoked.contains(&payload.nonce.as_str()) {
        return Err(Reject::Revoked);
    }
    match payload.bind.as_str() {
        // R9：候选集任一命中即通过。
        "fingerprint" => {
            if !fingerprints.iter().any(|f| f == &payload.sub) {
                return Err(Reject::DeviceMismatch);
            }
        }
        "order" => {}
        other => return Err(Reject::Invalid(format!("未知绑定方式：{}", other))),
    }
    Ok(payload)
}

// ──────────────────────── base32（RFC 4648，无填充，自实现不加新 crate） ────────────────────────

const B32_ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// base32 编码（无 `=` 填充，与 Python `base64.b32encode(...).rstrip(b'=')` 对齐）。
pub fn base32_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(5) * 8);
    let mut acc: u32 = 0;
    let mut nbits: u32 = 0;
    for &byte in data {
        acc = (acc << 8) | byte as u32;
        nbits += 8;
        while nbits >= 5 {
            nbits -= 5;
            out.push(B32_ALPHABET[((acc >> nbits) & 0x1F) as usize] as char);
        }
    }
    if nbits > 0 {
        out.push(B32_ALPHABET[((acc << (5 - nbits)) & 0x1F) as usize] as char);
    }
    out
}

/// base32 解码（容忍无填充输入；忽略空白与 `-`，大小写不敏感）。
pub fn base32_decode(s: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(s.len() * 5 / 8);
    let mut acc: u32 = 0;
    let mut nbits: u32 = 0;
    for ch in s.chars() {
        if ch.is_whitespace() || ch == '-' {
            continue;
        }
        let v = match ch.to_ascii_uppercase() {
            'A'..='Z' => ch.to_ascii_uppercase() as u32 - 'A' as u32,
            '2'..='7' => ch as u32 - '2' as u32 + 26,
            _ => return Err(format!("非法 base32 字符：{}", ch)),
        };
        acc = (acc << 5) | v;
        nbits += 5;
        if nbits >= 8 {
            nbits -= 8;
            out.push(((acc >> nbits) & 0xFF) as u8);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    /// 测试用密钥对（与 scripts/license/test_sk.hex 一致，仅用于测试，已公开无价值）。
    pub(crate) const TEST_SK_HEX: &str =
        "9d61b19deffebc3a4d0e9e36f34b7d1b3b47d5f9dddc11e5d6c9ecdd4ba1f74b";
    /// 由 ed25519-dalek 推导并回拍确认的测试公钥。
    pub(crate) const TEST_PK_B64: &str = "BTLe84VRdnkMFszsnEwqcS5EcmTSecw+jhD8Ad3HwRU=";

    pub(crate) fn test_signing_key() -> SigningKey {
        let bytes = hex_decode(TEST_SK_HEX);
        let arr: [u8; 32] = bytes.try_into().unwrap();
        SigningKey::from_bytes(&arr)
    }

    fn test_verifying_key() -> VerifyingKey {
        test_signing_key().verifying_key()
    }

    fn hex_decode(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    /// 用测试私钥签发 license（与 Python issue.py 相同的构造流程）。
    pub(crate) fn sign_license(payload: &LicensePayload) -> String {
        // canonical JSON：键序固定、无空白（与 Python sort_keys+separators 对齐）。
        let canonical = canonical_json(payload);
        let sk = test_signing_key();
        let sig = sk.sign(canonical.as_bytes());
        format!(
            "SLPRO-{}-{}",
            base32_encode(canonical.as_bytes()),
            base32_encode(&sig.to_bytes())
        )
    }

    /// canonical JSON（手工构造，字段序按字典序，与 Python `json.dumps(sort_keys=True)` 一致）。
    pub(crate) fn canonical_json(p: &LicensePayload) -> String {
        // 键按字典序：bind, exp, iat, nonce, seats, sku, sub, v
        let exp = match p.exp {
            Some(e) => e.to_string(),
            None => "null".into(),
        };
        format!(
            "{{\"bind\":\"{}\",\"exp\":{},\"iat\":{},\"nonce\":\"{}\",\"seats\":{},\"sku\":\"{}\",\"sub\":\"{}\",\"v\":{}}}",
            p.bind, exp, p.iat, p.nonce, p.seats, p.sku, p.sub, p.v
        )
    }

    pub(crate) fn sample_payload() -> LicensePayload {
        LicensePayload {
            v: 1,
            sku: "desktop-pro".into(),
            iat: 1_780_000_000,
            exp: None,
            sub: "FINGERPRIN".into(),
            bind: "order".into(),
            seats: 3,
            nonce: "NONCE123".into(),
        }
    }

    fn fps() -> Vec<String> {
        vec!["FINGERPRIN".into()]
    }

    fn validate_ok(raw: &str) -> Result<LicensePayload, Reject> {
        validate_token(raw, 1_780_000_100, &fps(), &[test_verifying_key()], &[], 1)
    }

    /// T3：Python issue.py 用测试私钥签发的 fixture license，
    /// Rust ed25519-dalek 必须验签通过（跨语言一致性闭环）。
    /// 与 scripts/license/test_fixture.json 逐字一致；roundtrip_check.py 负责再生成比对。
    #[test]
    fn python_fixture_license_validates() {
        const FIXTURE_LICENSE: &str = "SLPRO-PMRGE2LOMQRDUITGNFXGOZLSOBZGS3TUEIWCEZLYOARDU3TVNRWCYITJMF2CEORRG44DAMBQGAYDAMBMEJXG63TDMURDUISOJ5HEGRJRGIZSELBCONSWC5DTEI5DGLBCONVXKIR2EJSGK43LORXXALLQOJXSELBCON2WEIR2EJDESTSHIVJFAUSJJYRCYITWEI5DC7I-ZFBR2N3XXFRH5UKDBQB2CM6NOSYLNRPSZM6CGWOPHQRRLFQWWZJSBWTKZPPBP66ZQXLJONBRID2DKTN4GE5KLXYIY6PO7VQKL5OXUBY";
        let p = validate_token(
            FIXTURE_LICENSE,
            1_780_000_100,
            &fps(),
            &[test_verifying_key()],
            &[],
            1,
        )
        .expect("Python 签发的 fixture license 必须通过 Rust 验签");
        assert_eq!(p.sub, "FINGERPRIN");
        assert_eq!(p.bind, "fingerprint");
        assert_eq!(p.seats, 3);
        assert_eq!(p.iat, 1_780_000_000);
        assert_eq!(p.nonce, "NONCE123");
    }

    #[test]
    fn test_keypair_self_consistent() {
        // 测试密钥对自洽：dalek 推导公钥等于回拍值（同时校准 scripts/license 测试私钥）。
        use base64::{engine::general_purpose::STANDARD, Engine};
        let pk_b64 = STANDARD.encode(test_signing_key().verifying_key().to_bytes());
        assert_eq!(pk_b64, TEST_PK_B64);
    }

    // ── base32 ──

    #[test]
    fn base32_roundtrip() {
        let data = b"hello soundlink license";
        let enc = base32_encode(data);
        assert_eq!(base32_decode(&enc).unwrap(), data);
    }

    #[test]
    fn base32_matches_rfc4648_vectors() {
        // RFC 4648 测试向量（去填充）。
        assert_eq!(base32_encode(b""), "");
        assert_eq!(base32_encode(b"f"), "MY");
        assert_eq!(base32_encode(b"fo"), "MZXQ");
        assert_eq!(base32_encode(b"foo"), "MZXW6");
        assert_eq!(base32_encode(b"foob"), "MZXW6YQ");
        assert_eq!(base32_encode(b"fooba"), "MZXW6YTB");
        assert_eq!(base32_encode(b"foobar"), "MZXW6YTBOI");
        assert_eq!(base32_decode("MZXW6YTBOI").unwrap(), b"foobar");
        assert_eq!(base32_decode("mzxw6ytboi").unwrap(), b"foobar");
    }

    #[test]
    fn base32_decode_rejects_bad_char() {
        assert!(base32_decode("ABC0").is_err());
        assert!(base32_decode("ABC1").is_err());
        assert!(base32_decode("ABC=").is_err());
    }

    // ── 验签主路径（U1） ──

    #[test]
    fn valid_license_accepted() {
        let lic = sign_license(&sample_payload());
        let p = validate_ok(&lic).expect("合法 license 应通过");
        assert_eq!(p.sub, "FINGERPRIN");
        assert_eq!(p.seats, 3);
    }

    #[test]
    fn valid_license_with_whitespace_and_case() {
        let lic = sign_license(&sample_payload());
        // 模拟用户粘贴时带换行、空格、小写。
        let messy = format!("  {}\n ", lic.to_lowercase().replace('-', " -\n"));
        assert!(validate_ok(&messy).is_ok());
    }

    #[test]
    fn tampered_payload_rejected() {
        let lic = sign_license(&sample_payload());
        // 篡改 payload 段中间一个字符（保持合法 base32 字符集）。
        let mut chars: Vec<char> = lic.chars().collect();
        let idx = lic.len() / 3;
        chars[idx] = if chars[idx] == 'A' { 'B' } else { 'A' };
        let tampered: String = chars.into_iter().collect();
        // 解码可能仍成功，但签名或 JSON 解析必失败。
        assert!(matches!(validate_ok(&tampered), Err(Reject::Invalid(_))));
    }

    #[test]
    fn tampered_signature_rejected() {
        let payload = sample_payload();
        let lic = sign_license(&payload);
        let sig_start = lic.rfind('-').unwrap() + 1;
        let mut chars: Vec<char> = lic.chars().collect();
        chars[sig_start] = if chars[sig_start] == 'A' { 'B' } else { 'A' };
        let tampered: String = chars.into_iter().collect();
        assert!(matches!(validate_ok(&tampered), Err(Reject::Invalid(_))));
    }

    #[test]
    fn wrong_prefix_rejected() {
        let lic = sign_license(&sample_payload()).replacen("SLPRO", "SLMAX", 1);
        assert!(matches!(validate_ok(&lic), Err(Reject::Invalid(_))));
    }

    #[test]
    fn broken_base32_rejected() {
        assert!(matches!(
            validate_ok("SLPRO-!!!-AAA"),
            Err(Reject::Invalid(_))
        ));
    }

    #[test]
    fn broken_json_rejected() {
        // 合法 base32 但内容不是 JSON；需配合真签名才走到 JSON 解析，
        // 签名不过时同样 Invalid——此处验证前置签名拦截。
        let fake = format!("SLPRO-{}-{}", base32_encode(b"not json"), base32_encode(&[0u8; 64]));
        assert!(matches!(validate_ok(&fake), Err(Reject::Invalid(_))));
    }

    #[test]
    fn future_version_rejected() {
        let mut p = sample_payload();
        p.v = 2;
        let lic = sign_license(&p);
        assert!(matches!(validate_ok(&lic), Err(Reject::Invalid(_))));
    }

    #[test]
    fn zero_version_rejected() {
        let mut p = sample_payload();
        p.v = 0;
        let lic = sign_license(&p);
        assert!(matches!(validate_ok(&lic), Err(Reject::Invalid(_))));
    }

    #[test]
    fn older_version_accepted_under_higher_max() {
        // C2/R10：max=2 的实现下 v=1 的旧 key 仍通过。
        let mut p = sample_payload();
        p.v = 1;
        let lic = sign_license(&p);
        let r = validate_token(&lic, 1_780_000_100, &fps(), &[test_verifying_key()], &[], 2);
        assert!(r.is_ok());
    }

    #[test]
    fn expired_rejected() {
        let mut p = sample_payload();
        p.exp = Some(1_780_000_050);
        let lic = sign_license(&p);
        assert_eq!(validate_ok(&lic), Err(Reject::Expired));
    }

    #[test]
    fn not_yet_expired_accepted() {
        let mut p = sample_payload();
        p.exp = Some(1_780_000_100); // now == exp 时尚未过期（now > exp 才算）
        let lic = sign_license(&p);
        assert!(validate_ok(&lic).is_ok());
    }

    #[test]
    fn perpetual_no_exp_accepted() {
        // 买断：无 exp，且把系统时间调到遥远未来也必须通过（U6）。
        let lic = sign_license(&sample_payload());
        let r = validate_token(&lic, u64::MAX / 2, &fps(), &[test_verifying_key()], &[], 1);
        assert!(r.is_ok());
    }

    #[test]
    fn fingerprint_match_accepted() {
        let mut p = sample_payload();
        p.bind = "fingerprint".into();
        let lic = sign_license(&p);
        assert!(validate_ok(&lic).is_ok());
    }

    #[test]
    fn fingerprint_mismatch_rejected() {
        let mut p = sample_payload();
        p.bind = "fingerprint".into();
        p.sub = "OTHERDEVIC".into();
        let lic = sign_license(&p);
        assert_eq!(validate_ok(&lic), Err(Reject::DeviceMismatch));
    }

    #[test]
    fn revoked_nonce_rejected() {
        let lic = sign_license(&sample_payload());
        let r = validate_token(
            &lic,
            1_780_000_100,
            &fps(),
            &[test_verifying_key()],
            &["NONCE123"],
            1,
        );
        assert_eq!(r, Err(Reject::Revoked));
    }

    #[test]
    fn unknown_sku_rejected() {
        let mut p = sample_payload();
        p.sku = "desktop-pro-max".into();
        let lic = sign_license(&p);
        assert!(matches!(validate_ok(&lic), Err(Reject::Invalid(_))));
    }

    #[test]
    fn second_key_rotation_accepted() {
        // C1：公钥数组中任意一把命中即通过（模拟密钥轮换）。
        let other_sk = SigningKey::from_bytes(&[7u8; 32]);
        let canonical = canonical_json(&sample_payload());
        let sig = other_sk.sign(canonical.as_bytes());
        let lic = format!(
            "SLPRO-{}-{}",
            base32_encode(canonical.as_bytes()),
            base32_encode(&sig.to_bytes())
        );
        let keys = vec![test_verifying_key(), other_sk.verifying_key()];
        let r = validate_token(&lic, 1_780_000_100, &fps(), &keys, &[], 1);
        assert!(r.is_ok());
    }

    #[test]
    fn unknown_extra_fields_ignored() {
        // R10：payload 含未来新增字段时旧解析不报错（serde 忽略未知字段）。
        let raw = r#"{"bind":"order","exp":null,"iat":1780000000,"nonce":"NONCE123","seats":3,"sku":"desktop-pro","sub":"FINGERPRIN","tier":"ultra","v":1}"#;
        let sk = test_signing_key();
        let sig = sk.sign(raw.as_bytes());
        let lic = format!(
            "SLPRO-{}-{}",
            base32_encode(raw.as_bytes()),
            base32_encode(&sig.to_bytes())
        );
        let p = validate_ok(&lic).expect("含未知字段的旧 key 应通过");
        assert_eq!(p.sub, "FINGERPRIN");
    }

    #[test]
    fn missing_seats_defaults_to_three() {
        // C5：缺失 seats 的旧 key 取宽松默认（3）。
        let raw = r#"{"bind":"order","exp":null,"iat":1780000000,"nonce":"NONCE123","sku":"desktop-pro","sub":"FINGERPRIN","v":1}"#;
        let sk = test_signing_key();
        let sig = sk.sign(raw.as_bytes());
        let lic = format!(
            "SLPRO-{}-{}",
            base32_encode(raw.as_bytes()),
            base32_encode(&sig.to_bytes())
        );
        let p = validate_ok(&lic).expect("缺失 seats 应取默认值");
        assert_eq!(p.seats, 3);
    }

    #[test]
    fn vendor_pubkey_placeholder_decodes_to_32_bytes_or_is_replaced() {
        // R2：占位符被替换后，所有 vendor 公钥必须可解码为 32 字节。
        for b64 in PUBKEYS_VENDOR_B64 {
            if *b64 == "VENDOR_PUBLIC_KEY_PLACEHOLDER" {
                continue; // 占位符：keygen 后由作者替换。
            }
            use base64::{engine::general_purpose::STANDARD, Engine};
            let bytes = STANDARD.decode(b64).expect("vendor 公钥 base64 解码失败");
            assert_eq!(bytes.len(), 32, "vendor 公钥必须为 32 字节");
        }
    }
}
