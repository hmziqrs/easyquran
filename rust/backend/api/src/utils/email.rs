use hmac::{Hmac, Mac};
use sha2::Sha256;

/// Canonical account-email form: trimmed, lowercased.
///
/// Identity lookups (`find_by_email`), registration, email change and OAuth
/// linking all go through this so `Foo@X.com` and `foo@x.com` are one account.
/// Providers treat mailbox local parts case-insensitively in practice; the
/// stored form is what mail is delivered to.
pub fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
}

/// Canonical verification identity of an address (A10): the mailbox the account
/// is reachable at, with the spellings one mailbox accepts folded together.
///
/// - `gmail.com` and `googlemail.com` ignore dots in the local part, and
///   `googlemail.com` is the same mailbox as `gmail.com`.
/// - A `+tag` suffix is dropped for every domain.
///
/// Other providers' dot handling is not folded. Used only to count accounts per
/// mailbox for registration caps, never for lookups or delivery.
pub fn verification_identity(email: &str) -> String {
    let canonical = normalize_email(email);
    let Some((local, domain)) = canonical.rsplit_once('@') else {
        return canonical;
    };
    let local = local.split('+').next().unwrap_or_default();
    let (local, domain) = if matches!(domain, "gmail.com" | "googlemail.com") {
        (local.replace('.', ""), "gmail.com".to_string())
    } else {
        (local.to_string(), domain.to_string())
    };
    format!("{local}@{domain}")
}

type VerificationHmac = Hmac<Sha256>;

/// Keyed HMAC-SHA256 of [`verification_identity`], hex-encoded. This is an
/// identity key for account caps, not a Quran digest and not a SHA audit value.
pub fn verification_identity_hash(email: &str) -> String {
    let mut mac =
        VerificationHmac::new_from_slice(crate::config::settings::verification_identity_key())
            .expect("HMAC accepts any key length");
    mac.update(verification_identity(email).as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_and_lowercases() {
        assert_eq!(
            normalize_email("  Foo.Bar+Tag@Example.COM "),
            "foo.bar+tag@example.com"
        );
        assert_eq!(normalize_email("already@lower.com"), "already@lower.com");
        assert_eq!(normalize_email(""), "");
    }

    #[test]
    fn gmail_dots_and_plus_tags_fold_to_one_identity() {
        assert_eq!(verification_identity("a.b@gmail.com"), "ab@gmail.com");
        assert_eq!(verification_identity("ab@gmail.com"), "ab@gmail.com");
        assert_eq!(verification_identity("A.B+promo@GMail.com"), "ab@gmail.com");
        assert_eq!(verification_identity("a.b@googlemail.com"), "ab@gmail.com");
    }

    #[test]
    fn non_gmail_keeps_dots_but_drops_plus_tags() {
        assert_eq!(verification_identity("a.b@example.com"), "a.b@example.com");
        assert_eq!(verification_identity("ab+x@example.com"), "ab@example.com");
        assert_ne!(
            verification_identity("ab@example.com"),
            verification_identity("ab@gmail.com")
        );
    }

    #[test]
    fn identity_hash_matches_across_spellings_and_differs_by_mailbox() {
        assert_eq!(
            verification_identity_hash("a.b@gmail.com"),
            verification_identity_hash("ab@gmail.com")
        );
        assert_ne!(
            verification_identity_hash("ab@gmail.com"),
            verification_identity_hash("ab@example.com")
        );
        let h = verification_identity_hash("ab@gmail.com");
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
