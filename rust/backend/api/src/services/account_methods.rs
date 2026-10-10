//! Shared "keep at least one sign-in method" guard for account-management
//! flows (identity unlink, passkey removal). One definition so both call sites
//! agree on what counts as a method.

/// Every method that can sign the user in: each linked OAuth identity, a
/// password, and (at most) one passkey credit however many credentials exist.
pub fn total_sign_in_methods(
    identity_count: usize,
    has_password: bool,
    passkey_count: u64,
) -> usize {
    identity_count + usize::from(has_password) + usize::from(passkey_count > 0)
}

/// A user must keep at least one sign-in method; removing the last one is refused.
pub fn can_unlink(total_methods: usize) -> bool {
    total_methods.saturating_sub(1) >= 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_unlink_requires_a_surviving_method() {
        assert!(
            !can_unlink(0),
            "an account with no methods has none to unlink"
        );
        assert!(!can_unlink(1), "removing the last method must be refused");
        assert!(can_unlink(2));
        assert!(can_unlink(3));
    }

    #[test]
    fn total_sign_in_methods_counts_a_passkey_once() {
        assert_eq!(total_sign_in_methods(0, false, 0), 0);
        assert_eq!(total_sign_in_methods(1, false, 0), 1);
        assert_eq!(total_sign_in_methods(0, true, 0), 1);
        assert_eq!(
            total_sign_in_methods(0, false, 3),
            1,
            "any number of passkeys is one method"
        );
        assert_eq!(total_sign_in_methods(1, true, 1), 3);
    }
}
