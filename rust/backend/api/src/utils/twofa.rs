use chrono::{DateTime, FixedOffset, Utc};
use getrandom::getrandom;
use hmac::{Hmac, Mac};
use sha1::Sha1;

pub const DEFAULT_TOTP_STEP: u64 = 30;
pub const DEFAULT_TOTP_DIGITS: u32 = 6;

pub fn generate_secret_base32(num_bytes: usize) -> Option<String> {
    let mut buf = vec![0u8; num_bytes];
    getrandom(&mut buf).ok()?;
    Some(data_encoding::BASE32_NOPAD.encode(&buf))
}

pub fn build_otpauth_url(label: &str, issuer: &str, secret_base32: &str, digits: u32) -> String {
    let safe_label = urlencoding::encode(&format!("{}:{}", issuer, label)).into_owned();
    let safe_issuer = urlencoding::encode(issuer).into_owned();
    format!(
        "otpauth://totp/{}?secret={}&issuer={}&algorithm=SHA1&digits={}&period={}",
        safe_label, secret_base32, safe_issuer, digits, DEFAULT_TOTP_STEP
    )
}

pub fn generate_totp_code_at(
    secret_base32: &str,
    now: DateTime<FixedOffset>,
    step: u64,
    digits: u32,
) -> Option<String> {
    let secret = data_encoding::BASE32_NOPAD
        .decode(secret_base32.as_bytes())
        .ok()?;
    let counter = now.timestamp().div_euclid(step as i64) as u64;

    let mut msg = [0u8; 8];
    for (i, b) in counter.to_be_bytes().iter().enumerate() {
        msg[i] = *b;
    }

    let mut mac = Hmac::<Sha1>::new_from_slice(&secret).ok()?;
    mac.update(&msg);
    let hmac = mac.finalize().into_bytes();

    let offset = (hmac[19] & 0x0f) as usize;
    let bin_code = ((hmac[offset] as u32 & 0x7f) << 24)
        | ((hmac[offset + 1] as u32) << 16)
        | ((hmac[offset + 2] as u32) << 8)
        | (hmac[offset + 3] as u32);

    let modulo = pow10(digits);
    let code = bin_code % modulo;

    Some(format!("{:0width$}", code, width = digits as usize))
}

pub fn generate_totp_code_now(secret_base32: &str, digits: u32) -> Option<String> {
    generate_totp_code_at(
        secret_base32,
        Utc::now().fixed_offset(),
        DEFAULT_TOTP_STEP,
        digits,
    )
}

pub fn verify_totp_code_at(
    secret_base32: &str,
    code: &str,
    now: DateTime<FixedOffset>,
    step: u64,
    digits: u32,
    window: i64,
) -> Option<i64> {
    if code.len() != digits as usize || !code.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }

    let secret = match data_encoding::BASE32_NOPAD.decode(secret_base32.as_bytes()) {
        Ok(s) => s,
        Err(_) => return None,
    };

    let current_counter = now.timestamp().div_euclid(step as i64);

    for i in -window..=window {
        let counter = current_counter + i;

        let mut msg = [0u8; 8];
        for (idx, b) in (counter as u64).to_be_bytes().iter().enumerate() {
            msg[idx] = *b;
        }

        if let Some(candidate) = hmac_truncate_to_digits(&secret, &msg, digits) {
            if constant_time_eq(code.as_bytes(), candidate.as_bytes()) {
                return Some(counter);
            }
        }
    }

    None
}

pub fn verify_totp_code_now(secret_base32: &str, code: &str) -> Option<i64> {
    verify_totp_code_at(
        secret_base32,
        code,
        Utc::now().fixed_offset(),
        DEFAULT_TOTP_STEP,
        DEFAULT_TOTP_DIGITS,
        1,
    )
}

pub fn verify_totp_code_now_bool(secret_base32: &str, code: &str) -> bool {
    verify_totp_code_now(secret_base32, code).is_some()
}

pub fn is_fresh_counter(matched: i64, last: Option<i64>) -> bool {
    match last {
        None => true,
        Some(prev) => matched > prev,
    }
}

pub fn next_last(matched: i64, _last: Option<i64>) -> i64 {
    matched
}

pub fn generate_backup_codes(count: usize) -> Option<Vec<String>> {
    (0..count).map(|_| generate_backup_code()).collect()
}

pub fn hash_backup_codes(codes: &[String]) -> Vec<String> {
    codes.iter().map(|c| hash_backup_code(c)).collect()
}

pub fn consume_backup_code(hashed_codes: &[String], input_code: &str) -> Option<Vec<String>> {
    for (pos, stored) in hashed_codes.iter().enumerate() {
        if password_auth::verify_password(input_code, stored).is_ok() {
            let mut updated = hashed_codes.to_vec();
            updated.remove(pos);
            return Some(updated);
        }
    }
    None
}

fn generate_backup_code() -> Option<String> {
    const ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";

    let mut chars = [0u8; 12];
    for c in chars.iter_mut() {
        *c = ALPHABET[sample_index(ALPHABET.len())?];
    }

    Some(format!(
        "{}{}{}{}-{}{}{}{}-{}{}{}{}",
        chars[0] as char,
        chars[1] as char,
        chars[2] as char,
        chars[3] as char,
        chars[4] as char,
        chars[5] as char,
        chars[6] as char,
        chars[7] as char,
        chars[8] as char,
        chars[9] as char,
        chars[10] as char,
        chars[11] as char,
    ))
}

fn hash_backup_code(code: &str) -> String {
    password_auth::generate_hash(code)
}

// Rejection sampling, not `% len`: 256 isn't a multiple of the alphabet size,
// so modulo would skew the distribution and bias the backup codes.
fn sample_index(len: usize) -> Option<usize> {
    let limit = 256 - (256 % len);
    loop {
        let mut b = [0u8; 1];
        getrandom(&mut b).ok()?;
        if (b[0] as usize) < limit {
            return Some((b[0] as usize) % len);
        }
    }
}

fn hmac_truncate_to_digits(secret: &[u8], msg: &[u8; 8], digits: u32) -> Option<String> {
    let mut mac = Hmac::<Sha1>::new_from_slice(secret).ok()?;
    mac.update(msg);
    let hmac = mac.finalize().into_bytes();

    let offset = (hmac[19] & 0x0f) as usize;
    let bin_code = ((hmac[offset] as u32 & 0x7f) << 24)
        | ((hmac[offset + 1] as u32) << 16)
        | ((hmac[offset + 2] as u32) << 8)
        | (hmac[offset + 3] as u32);

    let modulo = pow10(digits);
    let code = bin_code % modulo;

    Some(format!("{:0width$}", code, width = digits as usize))
}

fn pow10(n: u32) -> u32 {
    let mut v = 1u32;
    for _ in 0..n {
        v = v.saturating_mul(10);
    }
    v
}

// Constant-time compare — don't replace with `==`, which leaks via timing
// whether backup/TOTP codes match.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    use subtle::ConstantTimeEq;
    a.ct_eq(b).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_secret_generation_is_base32() {
        let s = generate_secret_base32(20).expect("CSPRNG available");
        assert!(!s.is_empty());
        assert!(data_encoding::BASE32_NOPAD.decode(s.as_bytes()).is_ok());
    }

    #[test]
    fn test_totp_roundtrip_now() {
        let secret = generate_secret_base32(20).expect("CSPRNG available");
        let code = generate_totp_code_now(&secret, DEFAULT_TOTP_DIGITS).unwrap();
        assert!(verify_totp_code_now(&secret, &code).is_some());
        assert!(verify_totp_code_now_bool(&secret, &code));
    }

    #[test]
    fn test_verify_returns_current_counter_on_match() {
        let secret = generate_secret_base32(20).expect("CSPRNG available");
        let now = Utc.timestamp_opt(1_700_000_045, 0).unwrap().fixed_offset(); // 45s
        let code =
            generate_totp_code_at(&secret, now, DEFAULT_TOTP_STEP, DEFAULT_TOTP_DIGITS).unwrap();
        let matched = verify_totp_code_at(
            &secret,
            &code,
            now,
            DEFAULT_TOTP_STEP,
            DEFAULT_TOTP_DIGITS,
            1,
        )
        .expect("code should verify at its own instant");
        assert_eq!(matched, 1_700_000_045_i64 / 30);
    }

    #[test]
    fn test_verify_returns_none_on_wrong_code() {
        let secret = generate_secret_base32(20).expect("CSPRNG available");
        let now = Utc.timestamp_opt(1_700_000_045, 0).unwrap().fixed_offset();
        assert!(verify_totp_code_at(
            &secret,
            "000000",
            now,
            DEFAULT_TOTP_STEP,
            DEFAULT_TOTP_DIGITS,
            1
        )
        .is_none());
    }

    #[test]
    fn test_first_ever_verify_accepts_any_counter() {
        assert!(is_fresh_counter(1_000_000, None));
        assert!(is_fresh_counter(0, None));
        assert!(is_fresh_counter(i64::MAX, None));
    }

    #[test]
    fn test_replay_of_used_counter_is_rejected() {
        let last_after_first = next_last(1_000_000, None);
        assert_eq!(last_after_first, 1_000_000);

        assert!(!is_fresh_counter(1_000_000, Some(last_after_first)));
        assert!(!is_fresh_counter(999_999, Some(last_after_first)));
    }

    #[test]
    fn test_higher_counter_is_accepted_after_step_advances() {
        let last = Some(1_000_000_i64);
        assert!(is_fresh_counter(1_000_001, last));
        assert!(is_fresh_counter(1_000_500, last));
        assert!(is_fresh_counter(i64::MAX, last));
        assert_eq!(next_last(1_000_001, last), 1_000_001);
    }

    #[test]
    fn test_next_last_always_matches_accepted_counter() {
        assert_eq!(next_last(5, None), 5);
        assert_eq!(next_last(7, Some(5)), 7);
        assert_eq!(next_last(42, Some(100)), 42);
    }

    #[test]
    fn test_backup_codes_generation_and_hashing() {
        let codes = generate_backup_codes(5).expect("CSPRNG available");
        assert_eq!(codes.len(), 5);
        for c in &codes {
            assert_eq!(c.len(), 14); // 12 chars + 2 hyphens
            assert!(c
                .chars()
                .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '-'));
        }
        let hashes = hash_backup_codes(&codes);
        assert_eq!(hashes.len(), 5);
        for h in &hashes {
            assert!(
                h.starts_with("$argon2"),
                "expected Argon2id PHC string, got: {h}"
            );
        }

        let updated = consume_backup_code(&hashes, &codes[0]);
        assert!(updated.is_some());
        assert_eq!(updated.unwrap().len(), 4);

        let not_found = consume_backup_code(&hashes, "WRONG-CODE-0000");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_otpauth_url_format() {
        let url = build_otpauth_url("user@example.com", "Ruxlog", "SECRET", 6);
        assert!(url.starts_with("otpauth://totp/"));
        assert!(url.contains("issuer=Ruxlog"));
        assert!(url.contains("digits=6"));
        assert!(url.contains("period=30"));
    }
}
