use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// HMAC (not Argon2) so stored codes can be found via indexed `WHERE code_hash = ?`.
pub fn hash_code(secret_key: &[u8], code: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(secret_key).expect("HMAC accepts any key length");
    mac.update(code.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_deterministic() {
        let key = b"server-secret";
        assert_eq!(hash_code(key, "AB12cd"), hash_code(key, "AB12cd"));
    }

    #[test]
    fn hash_differs_by_code_and_key() {
        let k1 = b"server-secret";
        let k2 = b"other-secret";
        assert_ne!(hash_code(k1, "AAAAAA"), hash_code(k1, "BBBBBB"));
        assert_ne!(hash_code(k1, "AAAAAA"), hash_code(k2, "AAAAAA"));
    }

    #[test]
    fn hash_is_hex() {
        let h = hash_code(b"k", "code");
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(h.len(), 64);
    }
}
