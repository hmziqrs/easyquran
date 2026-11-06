//! Decision precedence and step-down rules (A22). Pure functions, no I/O.
//!
//! Precedence, highest first: account ban > grant > challenge > nothing.
//! A device signal may challenge and never bans. A device-triggered action caps
//! at L2. A shared-device finding steps the level down one and bars L6 (ban).

/// Ladder levels this module reasons about (see docs/plans/rules.md).
pub const LEVEL_CHALLENGE: u8 = 2;
pub const LEVEL_INTERIM_RESTRICTION: u8 = 5;

/// What the enforcement layer does for one subject, highest precedence first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    AccountBan,
    Grant,
    Challenge,
    Nothing,
}

/// Inputs to one decision. `device_signal` is advisory: it can reach `Challenge`
/// and no higher.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Signals {
    pub account_ban: bool,
    pub grant: bool,
    pub challenge: bool,
    pub device_signal: bool,
}

pub fn resolve(signals: Signals) -> Outcome {
    if signals.account_ban {
        return Outcome::AccountBan;
    }
    if signals.grant {
        return Outcome::Grant;
    }
    if signals.challenge || signals.device_signal {
        return Outcome::Challenge;
    }
    Outcome::Nothing
}

/// A device-triggered action never exceeds L2.
pub fn cap_device_level(requested: u8) -> u8 {
    requested.min(LEVEL_CHALLENGE)
}

/// A shared-device finding steps the level down one and never reaches L6 (ban).
pub fn shared_device_level(requested: u8) -> u8 {
    requested.saturating_sub(1).min(LEVEL_INTERIM_RESTRICTION)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_signal_alone_cannot_create_a_ban() {
        let outcome = resolve(Signals {
            device_signal: true,
            ..Signals::default()
        });
        assert_eq!(outcome, Outcome::Challenge);
        assert_ne!(outcome, Outcome::AccountBan);
    }

    #[test]
    fn account_ban_outranks_grant_outranks_challenge() {
        let all = Signals {
            account_ban: true,
            grant: true,
            challenge: true,
            device_signal: true,
        };
        assert_eq!(resolve(all), Outcome::AccountBan);
        let no_ban = Signals {
            account_ban: false,
            ..all
        };
        assert_eq!(resolve(no_ban), Outcome::Grant);
        let challenge_only = Signals {
            account_ban: false,
            grant: false,
            ..all
        };
        assert_eq!(resolve(challenge_only), Outcome::Challenge);
        assert_eq!(resolve(Signals::default()), Outcome::Nothing);
    }

    #[test]
    fn device_triggered_actions_cap_at_l2() {
        assert_eq!(cap_device_level(6), 2);
        assert_eq!(cap_device_level(3), 2);
        assert_eq!(cap_device_level(1), 1);
        assert_eq!(cap_device_level(0), 0);
    }

    #[test]
    fn shared_device_steps_down_one_and_bars_l6() {
        assert_eq!(shared_device_level(6), 5, "L6 ban is barred");
        assert_eq!(shared_device_level(7), 5, "L7 referral also steps to L5");
        assert_eq!(shared_device_level(3), 2);
        assert_eq!(shared_device_level(0), 0);
    }
}
