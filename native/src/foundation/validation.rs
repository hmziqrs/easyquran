//! Validation helpers shared by forms: each returns `Some(reason)` when the
//! value is invalid, `None` when it is fine, so `nsh-form` validators and
//! plain handlers consume them alike.

/// Whether `value` looks like an email: one `@`, a dot in the domain, no
/// spaces (a deliverability check is a service's job, not a form's).
pub fn email_error(value: &str) -> Option<&'static str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Some("Enter an email address");
    }
    let Some((local, domain)) = trimmed.split_once('@') else {
        return Some("An email address needs one @");
    };
    if local.is_empty() || !domain.contains('.') || domain.contains(' ') || domain.ends_with('.') {
        return Some("That does not look like an email address");
    }
    None
}

/// Whether a password is long enough to use here.
pub fn password_error(value: &str) -> Option<&'static str> {
    if value.len() < 8 {
        return Some("Use at least 8 characters");
    }
    None
}

/// Whether two password fields disagree.
pub fn password_confirm_error(password: &str, confirm: &str) -> Option<&'static str> {
    (password != confirm).then_some("Passwords do not match")
}

/// Whether a phone number holds enough digits (any punctuation allowed).
pub fn phone_error(value: &str) -> Option<&'static str> {
    let digits = value.chars().filter(char::is_ascii_digit).count();
    if digits < 7 {
        return Some("Enter at least 7 digits");
    }
    None
}

/// Whether a required field is empty.
pub fn required_error(value: &str) -> Option<&'static str> {
    value.trim().is_empty().then_some("This field is required")
}

/// A password's strength from 0 (empty) to 4 (length, lower, upper, digit
/// or symbol), for a meter, not a rule.
pub fn password_strength(value: &str) -> u8 {
    let mut score = 0;
    if value.len() >= 8 {
        score += 1;
    }
    if value.chars().any(char::is_lowercase) {
        score += 1;
    }
    if value.chars().any(char::is_uppercase) {
        score += 1;
    }
    if value
        .chars()
        .any(|c| c.is_ascii_digit() || !c.is_alphanumeric())
    {
        score += 1;
    }
    score
}

#[cfg(test)]
mod tests {
    use super::*;

    // Break seen: `email_error` returned `Some` always — failed on the "ada@example.com" case.
    #[test]
    fn email_rules_match_what_forms_need() {
        assert_eq!(email_error("ada@example.com"), None);
        assert!(email_error("").is_some());
        assert!(email_error("ada").is_some());
        assert!(email_error("ada@localhost").is_some());
        assert!(email_error("ada@ example.com").is_some());
        assert!(email_error("ada@example.com.").is_some());
    }

    // Break seen: `password_error` returned `Some` always — failed on `assert_eq!(password_error("longenough"), None)`.
    #[test]
    fn password_and_phone_have_floors() {
        assert_eq!(password_error("longenough"), None);
        assert!(password_error("short").is_some());
        assert_eq!(password_confirm_error("a", "a"), None);
        assert!(password_confirm_error("a", "b").is_some());
        assert_eq!(phone_error("+1 (555) 010-1234"), None);
        assert!(phone_error("555").is_some());
        assert_eq!(required_error("  "), Some("This field is required"));
    }

    // Break seen: `password_strength` returned 0 always — failed at the
    // `("abcdefgh"), 2` assertion.
    #[test]
    fn strength_scores_length_kinds() {
        assert_eq!(password_strength(""), 0);
        assert_eq!(password_strength("abcdefgh"), 2);
        assert_eq!(password_strength("Abcdefg1"), 4);
    }
}
