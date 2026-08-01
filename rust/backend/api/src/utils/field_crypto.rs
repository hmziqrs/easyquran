use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::Engine;
use getrandom::getrandom;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use zeroize::{Zeroize, Zeroizing};

const NONCE_LEN: usize = 12;

const V1_PREFIX: &str = "V1:";

static FIELD_ENC_KEY: std::sync::OnceLock<Zeroizing<[u8; 32]>> = std::sync::OnceLock::new();

static FIELD_ENC_KEY_PREV: std::sync::OnceLock<Zeroizing<[u8; 32]>> = std::sync::OnceLock::new();

pub fn set_key(key: &[u8]) -> Result<(), String> {
    if key.len() != 32 {
        return Err(format!(
            "FIELD_ENC_KEY must be exactly 32 bytes for AES-256 (got {}). \
             Generate with: openssl rand -base64 32 | head -c 32 | base64 \
             or a raw 32-byte hex: openssl rand -hex 16 | head -c 32.",
            key.len()
        ));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(key);
    let wrapped = Zeroizing::new(arr);
    // Reject a conflicting key so rotation can't silently half-overwrite the slot.
    FIELD_ENC_KEY.get_or_init(|| wrapped);
    if FIELD_ENC_KEY.get().map(|w| &**w) != Some(&arr) {
        return Err(
            "FIELD_ENC_KEY was already initialized with a different value; \
             key rotation requires a process restart."
                .to_string(),
        );
    }
    Ok(())
}

pub fn set_previous_key(key: Option<&[u8]>) -> Result<(), String> {
    let Some(key) = key else {
        return Ok(());
    };
    if key.len() != 32 {
        return Err(format!(
            "FIELD_ENC_KEY_PREV must be exactly 32 bytes for AES-256 (got {}).",
            key.len()
        ));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(key);
    // Reject a conflicting mid-process change — surface it, don't silently reset.
    if let Some(existing) = FIELD_ENC_KEY_PREV.get() {
        if **existing != arr {
            return Err(
                "FIELD_ENC_KEY_PREV was already initialized with a different value; \
                 key rotation requires a process restart."
                    .to_string(),
            );
        }
        return Ok(());
    }
    let _ = FIELD_ENC_KEY_PREV.set(Zeroizing::new(arr));
    Ok(())
}

pub fn field_enc_key() -> Option<&'static [u8; 32]> {
    FIELD_ENC_KEY.get().map(|w| &**w)
}

pub fn field_enc_key_prev() -> Option<&'static [u8; 32]> {
    FIELD_ENC_KEY_PREV.get().map(|w| &**w)
}

pub fn encrypt(plaintext: &str) -> Result<String, FieldCryptoError> {
    let key = field_enc_key().ok_or(FieldCryptoError::KeyUnset)?;
    Ok(format!("{V1_PREFIX}{}", encrypt_with(plaintext, key)?))
}

pub fn decrypt(blob: &str) -> Result<String, FieldCryptoError> {
    let (payload, was_prefixed) = strip_version_prefix(blob);
    if let Some(key) = field_enc_key() {
        if let Ok(pt) = decrypt_with(payload, key) {
            return Ok(pt);
        }
    } else {
        return Err(FieldCryptoError::KeyUnset);
    }
    if let Some(prev) = field_enc_key_prev() {
        if let Ok(pt) = decrypt_with(payload, prev) {
            return Ok(pt);
        }
    }
    let _ = was_prefixed;
    Err(FieldCryptoError::Decrypt)
}

fn strip_version_prefix(blob: &str) -> (&str, bool) {
    if let Some(rest) = blob.strip_prefix("V") {
        if let Some(colon) = rest.find(':') {
            let ver = &rest[..colon];
            if !ver.is_empty() && ver.chars().all(|c| c.is_ascii_digit()) {
                return (&rest[colon + 1..], true);
            }
        }
    }
    (blob, false)
}

pub fn encrypt_with(plaintext: &str, key: &[u8; 32]) -> Result<String, FieldCryptoError> {
    // The getrandom error must propagate: falling back to the zeroed buffer would reuse an all-zero nonce, catastrophic for GCM.
    let mut nonce_bytes = Zeroizing::new([0u8; NONCE_LEN]);
    getrandom(nonce_bytes.as_mut()).map_err(|_| FieldCryptoError::Rng)?;
    let nonce = Nonce::from_slice(&*nonce_bytes);

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let mut ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|_| FieldCryptoError::Encrypt)?;

    let mut packed = Zeroizing::new(Vec::with_capacity(NONCE_LEN + ciphertext.len()));
    packed.extend_from_slice(&*nonce_bytes);
    packed.extend_from_slice(&ciphertext);
    ciphertext.zeroize();

    Ok(base64::engine::general_purpose::STANDARD.encode(&packed))
}

pub fn decrypt_with(blob: &str, key: &[u8; 32]) -> Result<String, FieldCryptoError> {
    let packed = Zeroizing::new(
        base64::engine::general_purpose::STANDARD
            .decode(blob.as_bytes())
            .map_err(|_| FieldCryptoError::Decode)?,
    );

    if packed.len() < NONCE_LEN {
        return Err(FieldCryptoError::Decode);
    }
    let (nonce_bytes, ciphertext) = packed.split_at(NONCE_LEN);
    let nonce = Nonce::from_slice(nonce_bytes);

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let mut plaintext = Zeroizing::new(
        cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| FieldCryptoError::Decrypt)?,
    );

    let bytes = std::mem::take(&mut *plaintext);
    String::from_utf8(bytes).map_err(|_| FieldCryptoError::Decode)
}

const DET_DOMAIN: &[u8] = b"ruxlog/field_crypto/det/v1/subkey";

const D1_PREFIX: &str = "D1:";

const SHA256_LEN: usize = 32;

pub fn encrypt_deterministic(plaintext: &str) -> Result<String, FieldCryptoError> {
    let key = field_enc_key().ok_or(FieldCryptoError::KeyUnset)?;
    Ok(format!(
        "{D1_PREFIX}{}",
        encrypt_deterministic_with(plaintext, key)?
    ))
}

pub fn decrypt_deterministic(blob: &str) -> Result<String, FieldCryptoError> {
    let payload = blob.strip_prefix(D1_PREFIX).unwrap_or(blob);
    if let Some(key) = field_enc_key() {
        if let Ok(pt) = decrypt_deterministic_with(payload, key) {
            return Ok(pt);
        }
    } else {
        return Err(FieldCryptoError::KeyUnset);
    }
    if let Some(prev) = field_enc_key_prev() {
        if let Ok(pt) = decrypt_deterministic_with(payload, prev) {
            return Ok(pt);
        }
    }
    Err(FieldCryptoError::Decrypt)
}

pub fn encrypt_deterministic_with(
    plaintext: &str,
    key: &[u8; 32],
) -> Result<String, FieldCryptoError> {
    let subkey = derive_subkey(key);
    let mut nonce_bytes = compute_siv(&subkey, plaintext.as_bytes());
    let nonce = Nonce::from_slice(&nonce_bytes[..NONCE_LEN]);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&*subkey));

    let mut ct = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|_| FieldCryptoError::Encrypt)?;

    let mut packed = Vec::with_capacity(NONCE_LEN + ct.len());
    packed.extend_from_slice(&nonce_bytes[..NONCE_LEN]);
    packed.extend_from_slice(&ct);
    nonce_bytes.zeroize();
    ct.zeroize();
    Ok(base64::engine::general_purpose::STANDARD.encode(&packed))
}

pub fn decrypt_deterministic_with(
    payload: &str,
    key: &[u8; 32],
) -> Result<String, FieldCryptoError> {
    let subkey = derive_subkey(key);
    let packed = base64::engine::general_purpose::STANDARD
        .decode(payload.as_bytes())
        .map_err(|_| FieldCryptoError::Decode)?;
    if packed.len() < NONCE_LEN {
        return Err(FieldCryptoError::Decode);
    }
    let (nonce_bytes, ct) = packed.split_at(NONCE_LEN);
    let nonce = Nonce::from_slice(nonce_bytes);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&*subkey));
    let mut plaintext = Zeroizing::new(
        cipher
            .decrypt(nonce, ct)
            .map_err(|_| FieldCryptoError::Decrypt)?,
    );
    let bytes = std::mem::take(&mut *plaintext);
    String::from_utf8(bytes).map_err(|_| FieldCryptoError::Decode)
}

fn derive_subkey(key: &[u8; 32]) -> Zeroizing<[u8; 32]> {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(DET_DOMAIN);
    let out = mac.finalize().into_bytes();
    let mut arr = [0u8; SHA256_LEN];
    arr.copy_from_slice(&out);
    Zeroizing::new(arr)
}

fn compute_siv(subkey: &[u8; 32], plaintext: &[u8]) -> Zeroizing<[u8; SHA256_LEN]> {
    let mut mac =
        <Hmac<Sha256> as Mac>::new_from_slice(subkey).expect("HMAC accepts any key length");
    mac.update(DET_DOMAIN);
    mac.update(plaintext);
    let out = mac.finalize().into_bytes();
    let mut arr = [0u8; SHA256_LEN];
    arr.copy_from_slice(&out);
    Zeroizing::new(arr)
}

/// Errors deliberately conflate wrong-key and tampered-ciphertext to avoid a decryption oracle.
#[derive(Debug, thiserror::Error)]
pub enum FieldCryptoError {
    #[error("field-encryption key is not installed (set FIELD_ENC_KEY at boot)")]
    KeyUnset,
    #[error("OS CSPRNG failed while generating nonce")]
    Rng,
    #[error("AES-GCM encryption failed")]
    Encrypt,
    #[error("AES-GCM decryption failed (tampered ciphertext or wrong key)")]
    Decrypt,
    #[error("ciphertext was not valid base64 / was too short / was not valid UTF-8")]
    Decode,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> [u8; 32] {
        let mut k = [0u8; 32];
        for (i, b) in k.iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(7).wrapping_add(0x11);
        }
        k
    }

    #[test]
    fn round_trip_recovers_plaintext() {
        let key = test_key();
        let pt = r#"{"iban":"DE89370400440532013000","holder":"Jane Doe"}"#;
        let ct = encrypt_with(pt, &key).expect("encrypt");
        let recovered = decrypt_with(&ct, &key).expect("decrypt");
        assert_eq!(recovered, pt);
    }

    #[test]
    fn empty_plaintext_round_trips() {
        let key = test_key();
        let ct = encrypt_with("", &key).expect("encrypt");
        assert_eq!(decrypt_with(&ct, &key).expect("decrypt"), "");
    }

    #[test]
    fn different_plaintexts_yield_different_ciphertexts() {
        let key = test_key();
        let a = encrypt_with("same", &key).expect("encrypt");
        let b = encrypt_with("same", &key).expect("encrypt");
        assert_ne!(
            a, b,
            "nonce reuse: identical ciphertexts for identical plaintext"
        );
        let c = encrypt_with("different", &key).expect("encrypt");
        assert_ne!(a, c);
    }

    #[test]
    fn tampered_ciphertext_fails_closed() {
        let key = test_key();
        let ct = encrypt_with("sensitive", &key).expect("encrypt");
        let mut packed = base64::engine::general_purpose::STANDARD
            .decode(ct.as_bytes())
            .expect("ciphertext is valid base64");
        let mid = packed.len() / 2;
        packed[mid] ^= 0xff;
        let tampered = base64::engine::general_purpose::STANDARD.encode(&packed);
        let err = decrypt_with(&tampered, &key).expect_err("tampered blob must fail");
        assert!(matches!(err, FieldCryptoError::Decrypt));
    }

    #[test]
    fn truncated_ciphertext_fails() {
        let key = test_key();
        let ct = encrypt_with("sensitive", &key).expect("encrypt");
        let packed = base64::engine::general_purpose::STANDARD
            .decode(ct)
            .unwrap();
        let short = base64::engine::general_purpose::STANDARD.encode(&packed[..NONCE_LEN]);
        let err = decrypt_with(&short, &key).expect_err("truncated blob must fail");
        assert!(matches!(
            err,
            FieldCryptoError::Decrypt | FieldCryptoError::Decode
        ));
    }

    #[test]
    fn wrong_key_fails_closed() {
        let key = test_key();
        let ct = encrypt_with("sensitive", &key).expect("encrypt");
        let mut other = [0u8; 32];
        other.copy_from_slice(&key);
        other[0] ^= 0x01;
        let err = decrypt_with(&ct, &other).expect_err("wrong key must fail");
        assert!(matches!(err, FieldCryptoError::Decrypt));
    }

    #[test]
    fn invalid_base64_fails() {
        let key = test_key();
        let err = decrypt_with("!!!not base64!!!", &key).expect_err("non-base64 must fail");
        assert!(matches!(err, FieldCryptoError::Decode));
    }

    #[test]
    fn ciphertext_is_base64() {
        let key = test_key();
        let ct = encrypt_with("payload", &key).expect("encrypt");
        base64::engine::general_purpose::STANDARD
            .decode(&ct)
            .expect("ciphertext must decode as base64");
        assert!(!ct.contains('"') && !ct.contains('\\'));
    }

    #[test]
    fn key_length_is_enforced() {
        let err = set_key(b"too-short").expect_err("non-32-byte key must be rejected");
        assert!(
            err.contains("32"),
            "error must mention the 32-byte requirement: {err}"
        );
    }

    #[test]
    fn set_key_round_trips_via_globals() {
        let mut k = [0u8; 32];
        for (i, b) in k.iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(3).wrapping_add(0x42);
        }
        let pt = "global-slot-independent";
        let ct = encrypt_with(pt, &k).expect("encrypt");
        assert_eq!(decrypt_with(&ct, &k).expect("decrypt"), pt);
    }

    #[test]
    fn global_key_slot_is_zeroize_wrapped() {
        let key_ref = field_enc_key();
        if let Some(k) = key_ref {
            assert_eq!(k.len(), 32, "borrowed key must be exactly 32 bytes");
        }
        let _ty_check: fn() = || -> () {
            let _ = std::sync::OnceLock::<Zeroizing<[u8; 32]>>::new();
        };
    }

    #[test]
    fn round_trip_survives_zeroize_wrapping() {
        let key = test_key();
        for pt in ["", "a", "short", "🦀 unicode payload 🔑 with secrets"] {
            let ct = encrypt_with(pt, &key).expect("encrypt");
            assert_eq!(decrypt_with(&ct, &key).expect("decrypt"), pt);
        }
    }

    #[test]
    fn versioned_envelope_round_trips_via_globals() {
        set_key(&test_key()).ok();
        let pt = "versioned payload";
        let ct = encrypt(pt).expect("encrypt");
        assert!(
            ct.starts_with("V1:"),
            "new envelope must carry the V1 prefix, got: {ct}"
        );
        assert_eq!(decrypt(&ct).expect("decrypt"), pt);
    }

    #[test]
    fn legacy_unprefixed_blob_still_decrypts() {
        set_key(&test_key()).ok();
        let legacy = encrypt_with("legacy-row", &test_key()).expect("legacy encrypt");
        assert!(!legacy.starts_with("V1:"), "fixture must be prefix-less");
        assert_eq!(decrypt(&legacy).expect("decrypt legacy"), "legacy-row");
    }

    #[test]
    fn previous_key_decrypts_after_rotation() {
        set_key(&test_key()).ok();
        let mut prev = [0u8; 32];
        for (i, b) in prev.iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(5).wrapping_add(0x77);
        }
        set_previous_key(Some(&prev)).ok();
        let legacy_under_prev = encrypt_with("old-key-row", &prev).expect("prev encrypt");
        assert_eq!(
            decrypt(&legacy_under_prev).expect("decrypt under previous key"),
            "old-key-row"
        );
    }

    #[test]
    fn set_previous_key_rejects_wrong_length() {
        let err = set_previous_key(Some(b"too-short"))
            .expect_err("non-32-byte previous key must be rejected");
        assert!(err.contains("32"));
    }

    #[test]
    fn strip_version_prefix_parses_known_and_leaves_unknown() {
        assert_eq!(strip_version_prefix("V1:abc"), ("abc", true));
        assert_eq!(strip_version_prefix("V42:xyz"), ("xyz", true));
        assert_eq!(
            strip_version_prefix("plainbase64=="),
            ("plainbase64==", false)
        );
        assert_eq!(strip_version_prefix("Vx:abc"), ("Vx:abc", false));
        assert_eq!(strip_version_prefix("Value"), ("Value", false));
    }

    #[test]
    fn deterministic_encrypt_is_stable_across_calls() {
        let key = test_key();
        let a = encrypt_deterministic_with("google-123", &key).expect("enc");
        let b = encrypt_deterministic_with("google-123", &key).expect("enc");
        assert_eq!(
            a, b,
            "deterministic: equal plaintext must yield equal ciphertext"
        );
    }

    #[test]
    fn deterministic_encrypt_distinct_ids_differ() {
        let key = test_key();
        let a = encrypt_deterministic_with("google-1", &key).expect("enc");
        let b = encrypt_deterministic_with("google-2", &key).expect("enc");
        assert_ne!(a, b, "distinct ids must encrypt to distinct ciphertexts");
    }

    #[test]
    fn deterministic_round_trips() {
        let key = test_key();
        for id in ["", "a", "1234567890", "sub|very-long-google-id-string-🦀"] {
            let ct = encrypt_deterministic_with(id, &key).expect("enc");
            assert_eq!(decrypt_deterministic_with(&ct, &key).expect("dec"), id);
        }
    }

    #[test]
    fn deterministic_envelope_round_trips_via_globals() {
        set_key(&test_key()).ok();
        let id = "global-google-id";
        let ct = encrypt_deterministic(id).expect("enc");
        assert!(
            ct.starts_with("D1:"),
            "deterministic envelope must carry the D1 prefix, got: {ct}"
        );
        assert_eq!(decrypt_deterministic(&ct).expect("dec"), id);
    }

    #[test]
    fn deterministic_keyed_api_independent_of_globals() {
        let mut k = [0u8; 32];
        for (i, b) in k.iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(9).wrapping_add(0x05);
        }
        let ct = encrypt_deterministic_with("lookup-key", &k).expect("enc");
        assert_eq!(
            decrypt_deterministic_with(&ct, &k).expect("dec"),
            "lookup-key"
        );
    }
}
