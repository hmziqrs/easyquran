use base64::Engine;
use http::HeaderMap;
use hmac::{Hmac, Mac};
use sha2::Sha256;

pub const MAX_SKEW_SECS: i64 = 5 * 60;

type HmacSha256 = Hmac<Sha256>;

pub fn hmac_sha256_hex(key: &[u8], msg: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(msg);
    hex::encode(mac.finalize().into_bytes())
}

pub fn verify_hmac_sha256_hex(key: &[u8], msg: &[u8], provided_hex: &str) -> bool {
    let expected = hmac_sha256_hex(key, msg);
    ct_eq(expected.as_bytes(), provided_hex.trim().as_bytes())
}

pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    use subtle::ConstantTimeEq;
    a.ct_eq(b).into()
}

pub fn header_str(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

pub fn timestamp_fresh(ts_secs: i64, now_secs: i64) -> bool {
    ts_secs.saturating_sub(now_secs).abs() <= MAX_SKEW_SECS
}

pub fn standard_webhooks_key(secret: &str) -> Vec<u8> {
    let trimmed = secret.strip_prefix("whsec_").unwrap_or(secret);
    match base64::engine::general_purpose::STANDARD.decode(trimmed) {
        Ok(key) => key,
        Err(_) => trimmed.as_bytes().to_vec(),
    }
}

pub fn verify_standard_webhooks(
    headers: &HeaderMap,
    secret: &str,
    body: &[u8],
    now_secs: i64,
) -> bool {
    let webhook_id = match header_str(headers, "webhook-id") {
        Some(v) => v,
        None => return false,
    };
    let webhook_ts = match header_str(headers, "webhook-timestamp") {
        Some(v) => v,
        None => return false,
    };
    let webhook_sig = match header_str(headers, "webhook-signature") {
        Some(v) => v,
        None => return false,
    };

    let ts_secs: i64 = match webhook_ts.parse() {
        Ok(n) => n,
        Err(_) => return false,
    };
    if !timestamp_fresh(ts_secs, now_secs) {
        return false;
    }

    let key = standard_webhooks_key(secret);
    let mut mac = match HmacSha256::new_from_slice(&key) {
        Ok(m) => m,
        // Fail closed here: an empty configured secret must reject, not panic (DoS).
        Err(_) => return false,
    };
    mac.update(webhook_id.as_bytes());
    mac.update(b".");
    mac.update(webhook_ts.as_bytes());
    mac.update(b".");
    mac.update(body);
    let expected = base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes());

    for entry in webhook_sig.split_whitespace() {
        let candidate = entry
            .strip_prefix("v1,")
            .or_else(|| entry.strip_prefix("v1="));
        if let Some(sig) = candidate {
            if ct_eq(sig.as_bytes(), expected.as_bytes()) {
                return true;
            }
        }
    }
    false
}

pub fn verify_ed25519(public_key: &[u8; 32], message: &[u8], signature: &[u8; 64]) -> bool {
    use ed25519_dalek::{Signature, Verifier, VerifyingKey};

    let vk = match VerifyingKey::from_bytes(public_key) {
        Ok(k) => k,
        Err(_) => return false,
    };
    let sig = Signature::from_bytes(signature);
    vk.verify(message, &sig).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::HeaderMap;

    #[test]
    fn hmac_known_answer_rfc4231_case1() {
        let key = [0x0bu8; 20];
        let tag = hmac_sha256_hex(&key, b"Hi There");
        assert_eq!(
            tag,
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
    }

    #[test]
    fn hmac_verify_accepts_correct_tag() {
        let key = b"whsec_test";
        let msg = b"{\"id\":\"evt_1\"}";
        let tag = hmac_sha256_hex(key, msg);
        assert!(verify_hmac_sha256_hex(key, msg, &tag));
    }

    #[test]
    fn hmac_verify_rejects_tampered_message() {
        let key = b"whsec_test";
        let tag = hmac_sha256_hex(key, b"original");
        assert!(!verify_hmac_sha256_hex(key, b"tampered", &tag));
    }

    #[test]
    fn hmac_verify_rejects_wrong_key() {
        let msg = b"body";
        let tag = hmac_sha256_hex(b"secret-a", msg);
        assert!(!verify_hmac_sha256_hex(b"secret-b", msg, &tag));
    }

    #[test]
    fn hmac_verify_rejects_bad_hex_and_length_mismatch() {
        let key = b"k";
        let msg = b"m";
        assert!(!verify_hmac_sha256_hex(key, msg, "nothex!!"));
        let full = hmac_sha256_hex(key, msg);
        assert!(!verify_hmac_sha256_hex(key, msg, &full[..10]));
    }

    #[test]
    fn ct_eq_equal_and_unequal() {
        assert!(ct_eq(b"abcdef", b"abcdef"));
        assert!(!ct_eq(b"abcdef", b"abcdeg"));
        assert!(!ct_eq(b"abc", b"abcdef")); // length mismatch → false
    }

    #[test]
    fn timestamp_fresh_within_window() {
        assert!(timestamp_fresh(1_000_000, 1_000_000));
        assert!(timestamp_fresh(1_000_000 + MAX_SKEW_SECS, 1_000_000));
        assert!(timestamp_fresh(1_000_000 - MAX_SKEW_SECS, 1_000_000));
    }

    #[test]
    fn timestamp_fresh_rejects_replay_outside_window() {
        assert!(!timestamp_fresh(1_000_000 + MAX_SKEW_SECS + 1, 1_000_000));
        assert!(!timestamp_fresh(1_000_000 - MAX_SKEW_SECS - 1, 1_000_000));
    }

    #[test]
    fn header_str_present_and_absent() {
        let mut h = HeaderMap::new();
        h.insert("Stripe-Signature", "t=1,v1=abc".parse().unwrap());
        assert_eq!(
            header_str(&h, "Stripe-Signature").as_deref(),
            Some("t=1,v1=abc")
        );
        assert!(header_str(&h, "Missing").is_none());
    }

    #[test]
    fn standard_webhooks_key_decodes_whsec_prefix() {
        let raw = [0x11u8; 32];
        let secret = format!(
            "whsec_{}",
            base64::engine::general_purpose::STANDARD.encode(raw)
        );
        assert_eq!(standard_webhooks_key(&secret), raw.to_vec());
    }

    #[test]
    fn standard_webhooks_key_accepts_bare_base64() {
        let raw = [0x22u8; 32];
        let secret = base64::engine::general_purpose::STANDARD.encode(raw);
        assert_eq!(standard_webhooks_key(&secret), raw.to_vec());
    }

    #[test]
    fn standard_webhooks_key_falls_back_to_raw_bytes() {
        let s = "not-base64!!";
        assert_eq!(standard_webhooks_key(s), s.as_bytes().to_vec());
    }

    #[test]
    fn standard_webhooks_verifies_and_rejects() {
        use base64::engine::general_purpose::STANDARD;
        let raw_key = [7u8; 32];
        let secret = format!("whsec_{}", STANDARD.encode(raw_key));
        let now = 1_700_000_000i64;
        let id = "evt_1";
        let body = br#"{"type":"x"}"#;

        let mut mac = HmacSha256::new_from_slice(&raw_key).unwrap();
        mac.update(format!("{id}.{now}.").as_bytes());
        mac.update(body);
        let sig = STANDARD.encode(mac.finalize().into_bytes());

        let mut h = HeaderMap::new();
        h.insert("webhook-id", id.parse().unwrap());
        h.insert("webhook-timestamp", now.to_string().parse().unwrap());
        h.insert("webhook-signature", format!("v1,{sig}").parse().unwrap());

        assert!(verify_standard_webhooks(&h, &secret, body, now));
        assert!(verify_standard_webhooks(
            &h,
            &secret,
            body,
            now + MAX_SKEW_SECS
        ));

        assert!(!verify_standard_webhooks(
            &h,
            &secret,
            body,
            now + MAX_SKEW_SECS + 1
        ));

        assert!(!verify_standard_webhooks(
            &h,
            &secret,
            b"{\"type\":\"y\"}",
            now
        ));

        let mut h2 = h.clone();
        h2.remove("webhook-signature");
        assert!(!verify_standard_webhooks(&h2, &secret, body, now));

        let mut mac2 = HmacSha256::new_from_slice(&[9u8; 32]).unwrap();
        mac2.update(format!("{id}.{now}.").as_bytes());
        mac2.update(body);
        let sig2 = STANDARD.encode(mac2.finalize().into_bytes());
        let mut h3 = HeaderMap::new();
        h3.insert("webhook-id", id.parse().unwrap());
        h3.insert("webhook-timestamp", now.to_string().parse().unwrap());
        h3.insert(
            "webhook-signature",
            format!("v1,{sig2} v1,{sig}").parse().unwrap(),
        );
        assert!(verify_standard_webhooks(&h3, &secret, body, now));
    }

    #[test]
    fn ed25519_accepts_valid_signature() {
        use ed25519_dalek::{Signer, SigningKey};

        let seed = [7u8; 32]; // deterministic test key
        let sk = SigningKey::from_bytes(&seed);
        let vk = sk.verifying_key();
        let msg = b"1700000000{ \"data\": { \"id\": \"txn_1\" } }";
        let sig = sk.sign(msg);
        assert!(verify_ed25519(&vk.to_bytes(), msg, &sig.to_bytes()));
    }

    #[test]
    fn ed25519_rejects_tampered_message_and_wrong_key() {
        use ed25519_dalek::{Signer, SigningKey};

        let sk = SigningKey::from_bytes(&[7u8; 32]);
        let vk = sk.verifying_key();
        let sig = sk.sign(b"1700000000body").to_bytes();

        assert!(!verify_ed25519(&vk.to_bytes(), b"1700000000BYTEM", &sig));
        let other = SigningKey::from_bytes(&[99u8; 32])
            .verifying_key()
            .to_bytes();
        assert!(!verify_ed25519(&other, b"1700000000body", &sig));
        assert!(!verify_ed25519(&[0u8; 32], b"1700000000body", &sig));
    }
}
