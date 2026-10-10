/// Canonical account-email form: trimmed, lowercased.
///
/// Identity lookups (`find_by_email`), registration, email change and OAuth
/// linking all go through this so `Foo@X.com` and `foo@x.com` are one account.
/// Providers treat mailbox local parts case-insensitively in practice; the
/// stored form is what mail is delivered to.
pub fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
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
}
