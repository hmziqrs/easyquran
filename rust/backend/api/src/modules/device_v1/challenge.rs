//! Device challenges (devices.md D3, step 5). One outstanding nonce per session,
//! single use, five-minute TTL. Process-local like the passkey state: a restart
//! means the client asks again. A full map refuses new challenges instead of
//! evicting a live one (A13's rule).

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use subtle::ConstantTimeEq;

use super::keys::random_b64;

pub const CHALLENGE_TTL: Duration = Duration::from_secs(300);
pub const MAX_CHALLENGES: usize = 10_000;

#[derive(Debug, PartialEq, Eq)]
pub enum ChallengeError {
    /// The map is full of live challenges. Retry after the TTL.
    Full,
    /// Missing, expired, already used, wrong user or wrong nonce. Not told apart.
    Invalid,
}

struct Pending {
    user_id: i32,
    nonce: String,
    issued: Instant,
}

pub struct ChallengeStore {
    cap: usize,
    pending: Mutex<HashMap<String, Pending>>,
}

impl ChallengeStore {
    pub fn with_cap(cap: usize) -> Self {
        Self {
            cap,
            pending: Mutex::new(HashMap::new()),
        }
    }

    /// Issue the session's nonce. A new one replaces the session's old one.
    pub fn issue(
        &self,
        session: &str,
        user_id: i32,
        now: Instant,
    ) -> Result<String, ChallengeError> {
        let nonce = random_b64();
        let mut map = self.pending.lock().map_err(|_| ChallengeError::Full)?;
        map.retain(|_, p| now.saturating_duration_since(p.issued) < CHALLENGE_TTL);
        if !map.contains_key(session) && map.len() >= self.cap {
            return Err(ChallengeError::Full);
        }
        map.insert(
            session.to_string(),
            Pending {
                user_id,
                nonce: nonce.clone(),
                issued: now,
            },
        );
        Ok(nonce)
    }

    /// Single use: the entry is removed on every attempt, whether or not it matches.
    pub fn consume(
        &self,
        session: &str,
        user_id: i32,
        nonce: &str,
        now: Instant,
    ) -> Result<(), ChallengeError> {
        let taken = {
            let mut map = self.pending.lock().map_err(|_| ChallengeError::Invalid)?;
            map.remove(session)
        };
        let pending = taken.ok_or(ChallengeError::Invalid)?;
        if pending.user_id != user_id
            || now.saturating_duration_since(pending.issued) >= CHALLENGE_TTL
        {
            return Err(ChallengeError::Invalid);
        }
        let same = pending.nonce.as_bytes().ct_eq(nonce.as_bytes());
        if bool::from(same) {
            Ok(())
        } else {
            Err(ChallengeError::Invalid)
        }
    }
}

static STORE: OnceLock<ChallengeStore> = OnceLock::new();

pub fn store() -> &'static ChallengeStore {
    STORE.get_or_init(|| ChallengeStore::with_cap(MAX_CHALLENGES))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store_with(cap: usize) -> ChallengeStore {
        ChallengeStore::with_cap(cap)
    }

    #[test]
    fn a_nonce_is_single_use() {
        let s = store_with(10);
        let now = Instant::now();
        let nonce = s.issue("sess-a", 1, now).unwrap();
        assert!(s.consume("sess-a", 1, &nonce, now).is_ok());
        assert_eq!(
            s.consume("sess-a", 1, &nonce, now),
            Err(ChallengeError::Invalid)
        );
    }

    #[test]
    fn an_expired_nonce_fails() {
        let s = store_with(10);
        let now = Instant::now();
        let nonce = s.issue("sess-b", 1, now).unwrap();
        let later = now + CHALLENGE_TTL + Duration::from_secs(1);
        assert_eq!(
            s.consume("sess-b", 1, &nonce, later),
            Err(ChallengeError::Invalid)
        );
    }

    #[test]
    fn a_wrong_nonce_or_wrong_user_fails_and_burns_the_challenge() {
        let s = store_with(10);
        let now = Instant::now();
        let nonce = s.issue("sess-c", 1, now).unwrap();
        assert_eq!(
            s.consume("sess-c", 1, "not-the-nonce", now),
            Err(ChallengeError::Invalid)
        );
        assert_eq!(
            s.consume("sess-c", 1, &nonce, now),
            Err(ChallengeError::Invalid)
        );

        let other = s.issue("sess-d", 1, now).unwrap();
        assert_eq!(
            s.consume("sess-d", 2, &other, now),
            Err(ChallengeError::Invalid)
        );
    }

    #[test]
    fn a_nonce_is_bound_to_its_session() {
        let s = store_with(10);
        let now = Instant::now();
        let nonce = s.issue("sess-e", 1, now).unwrap();
        assert_eq!(
            s.consume("sess-other", 1, &nonce, now),
            Err(ChallengeError::Invalid)
        );
        // The stranger's attempt spent nothing: the owner still consumes it once.
        assert!(s.consume("sess-e", 1, &nonce, now).is_ok());
    }

    #[test]
    fn a_new_nonce_replaces_the_sessions_old_one() {
        let s = store_with(10);
        let now = Instant::now();
        let first = s.issue("sess-f", 1, now).unwrap();
        let second = s.issue("sess-f", 1, now).unwrap();
        assert_eq!(
            s.consume("sess-f", 1, &first, now),
            Err(ChallengeError::Invalid)
        );
        assert!(s.consume("sess-f", 1, &second, now).is_err());
    }

    #[test]
    fn a_full_map_refuses_instead_of_evicting() {
        let s = store_with(2);
        let now = Instant::now();
        s.issue("sess-1", 1, now).unwrap();
        let second = s.issue("sess-2", 1, now).unwrap();
        assert_eq!(s.issue("sess-3", 1, now), Err(ChallengeError::Full));
        // A live session can still refresh its own nonce at the cap.
        assert!(s.issue("sess-1", 1, now).is_ok());
        // The refused attempt evicted nothing: sess-2 still consumes.
        assert!(s.consume("sess-2", 1, &second, now).is_ok());
    }

    #[test]
    fn expired_entries_make_room_at_the_cap() {
        let s = store_with(1);
        let now = Instant::now();
        s.issue("old", 1, now).unwrap();
        let later = now + CHALLENGE_TTL + Duration::from_secs(1);
        assert!(s.issue("new", 1, later).is_ok());
    }
}
