use validator::ValidationError;

use crate::config::settings::email_aliases_enabled;

/// Plus-tag detection: a `+` in the local part (`user+tag@domain`). A `+` in
/// the domain is not local-part aliasing and is already rejected by the email
/// regex's domain character class.
pub fn has_alias_tag(email: &str) -> bool {
    email.split('@').next().unwrap_or_default().contains('+')
}

/// Pure alias-policy check; env-reading wrapper is [`validate_email_alias`].
pub fn check_email_alias(email: &str, aliases_enabled: bool) -> Result<(), ValidationError> {
    if !aliases_enabled && has_alias_tag(email) {
        return Err(ValidationError::new("email_alias_not_allowed"));
    }
    Ok(())
}

/// Alias policy for every payload that sets an account email: plus-tagged
/// addresses are allowed while `EMAIL_ALIASES_ENABLED` (or the production
/// default) says they are on.
pub fn validate_email_alias(email: &str) -> Result<(), ValidationError> {
    check_email_alias(email, email_aliases_enabled())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_plus_only_in_local_part() {
        assert!(has_alias_tag("user+tag@example.com"));
        assert!(has_alias_tag("a+b.c@sub.example.com"));
        assert!(!has_alias_tag("user@example.com"));
        assert!(!has_alias_tag("first.last@example.com"));
        assert!(!has_alias_tag("user@plus+domain.example"));
    }

    #[test]
    fn gate_allows_when_enabled_and_rejects_when_disabled() {
        assert!(check_email_alias("user+tag@example.com", true).is_ok());
        assert!(check_email_alias("user@example.com", true).is_ok());
        assert!(check_email_alias("user@example.com", false).is_ok());

        let err = check_email_alias("user+tag@example.com", false).unwrap_err();
        assert_eq!(err.code, "email_alias_not_allowed");
    }
}
