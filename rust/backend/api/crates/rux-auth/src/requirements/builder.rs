use chrono::Duration;

#[derive(Debug, Clone, Default)]
pub struct AuthRequirements {
    pub(crate) authenticated: Option<bool>,
    pub(crate) verified: bool,
    pub(crate) unverified: bool,
    pub(crate) not_banned: bool,
    pub(crate) min_role: Option<i32>,
    pub(crate) ban_cache_duration: Duration,
}

impl AuthRequirements {
    pub fn new() -> Self {
        Self {
            ban_cache_duration: Duration::minutes(5),
            ..Default::default()
        }
    }

    pub fn authenticated(mut self) -> Self {
        self.authenticated = Some(true);
        self
    }

    pub fn unauthenticated(mut self) -> Self {
        self.authenticated = Some(false);
        self
    }

    pub fn verified(mut self) -> Self {
        self.verified = true;
        self
    }

    pub fn unverified(mut self) -> Self {
        self.unverified = true;
        self
    }

    pub fn totp_verified(self) -> Self {
        self
    }

    pub fn totp_if_enabled(self) -> Self {
        self
    }

    pub fn reauth_within(self, _duration: Duration) -> Self {
        self
    }

    pub fn not_banned(mut self) -> Self {
        self.not_banned = true;
        self
    }

    pub fn role_min(mut self, level: i32) -> Self {
        self.min_role = Some(level);
        self
    }

    pub fn ban_cache_duration(mut self, duration: Duration) -> Self {
        self.ban_cache_duration = duration;
        self
    }

    pub fn requires_auth(&self) -> bool {
        self.authenticated == Some(true)
    }

    pub fn requires_unauth(&self) -> bool {
        self.authenticated == Some(false)
    }
}

pub fn auth_requirements() -> AuthRequirements {
    AuthRequirements::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_authenticated_requirement() {
        let req = auth_requirements().authenticated();
        assert_eq!(req.authenticated, Some(true));
        assert!(req.requires_auth());
        assert!(!req.requires_unauth());
    }

    #[test]
    fn test_unauthenticated_requirement() {
        let req = auth_requirements().unauthenticated();
        assert_eq!(req.authenticated, Some(false));
        assert!(!req.requires_auth());
        assert!(req.requires_unauth());
    }

    #[test]
    fn test_chained_requirements() {
        let req = auth_requirements()
            .authenticated()
            .verified()
            .not_banned()
            .role_min(3);

        assert_eq!(req.authenticated, Some(true));
        assert!(req.verified);
        assert!(req.not_banned);
        assert_eq!(req.min_role, Some(3));
    }

    #[test]
    fn test_inverse_requirements() {
        let req = auth_requirements().authenticated().unverified();

        assert_eq!(req.authenticated, Some(true));
        assert!(req.unverified);
        assert!(!req.verified);
    }

    #[test]
    fn test_stepup_builders_are_noops() {
        let strict = auth_requirements().totp_verified();
        assert!(!strict.requires_auth());

        let conditional = auth_requirements().totp_if_enabled();
        assert!(!conditional.requires_auth());

        let req = auth_requirements().reauth_within(Duration::minutes(5));
        assert!(!req.requires_auth());
    }
}
