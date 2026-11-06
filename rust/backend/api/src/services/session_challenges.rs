//! Challenge slots kept in the caller's session payload (A13).
//!
//! Each flow owns one session key holding a map of handle -> slot. Slots live in the
//! session store, so they survive a process restart and are bound to that session's
//! cookie. A session holds at most `Quota::per_session` live slots; a full session is
//! refused, never evicted. Expired slots are pruned on every read and write.
//!
//! Single-use: `take` removes the slot before returning it. Read-modify-write runs under
//! [`lock`], so two requests in one process cannot both take the same handle.

use std::collections::BTreeMap;

use chrono::Utc;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tower_sessions::Session;
use tracing::{error, warn};

use crate::error::{ErrorCode, ErrorResponse};

#[derive(Clone, Copy, Debug)]
pub struct Quota {
    pub per_session: usize,
    pub ttl_secs: i64,
}

#[derive(Serialize, Deserialize)]
struct Slot {
    issued_at: i64,
    body: String,
    /// Session id the slot was issued under. A session rotation (`cycle_id`) keeps the payload
    /// but changes the id, so slots from before the rotation no longer match. Missing on
    /// payloads written before this field existed, which reads as no match.
    #[serde(default)]
    session_id: String,
}

fn current_id(session: &Session) -> String {
    session.id().map(|id| id.to_string()).unwrap_or_default()
}

type Slots = BTreeMap<String, Slot>;

static SESSION_CHALLENGE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Serialises read-modify-write on session payloads for single-use flows. Callers must not
/// hold the guard while calling another function that takes it.
pub async fn lock() -> tokio::sync::MutexGuard<'static, ()> {
    SESSION_CHALLENGE_LOCK.lock().await
}

fn session_error<E: std::fmt::Debug>(err: E) -> ErrorResponse {
    error!(error = ?err, "session challenge store access failed");
    ErrorResponse::new(ErrorCode::InternalServerError).with_message("Unable to store challenge")
}

/// One-way fingerprint for secret material kept in a session payload, so the session row
/// never holds a password hash or TOTP seed in the clear. Compare with `ct_eq` on the hex.
pub fn fingerprint(value: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(b"easyquran.session-challenge.v1\0");
    hasher.update(value.as_bytes());
    hex::encode(hasher.finalize())
}

/// Reads a session value. A payload that no longer decodes (for example one written before
/// a field existed) reads as absent, so the flow fails closed instead of returning a 500.
pub async fn read_payload<T: DeserializeOwned>(session: &Session, key: &str) -> Option<T> {
    match session.get::<T>(key).await {
        Ok(value) => value,
        Err(err) => {
            warn!(error = ?err, key, "unreadable session payload treated as absent");
            None
        }
    }
}

async fn load(session: &Session, key: &str, quota: Quota, now: i64, sid: &str) -> Slots {
    let stored: Option<Slots> = read_payload(session, key).await;
    stored
        .unwrap_or_default()
        .into_iter()
        .filter(|(_, slot)| now - slot.issued_at < quota.ttl_secs && slot.session_id == sid)
        .collect()
}

/// Stores `body` under `handle`. Refuses with `RateLimited` when the session is full.
pub async fn insert(
    session: &Session,
    key: &str,
    quota: Quota,
    handle: &str,
    body: String,
) -> Result<(), ErrorResponse> {
    let _guard = lock().await;
    if session.id().is_none() {
        // Creates the session row (and so the id the slot is bound to).
        session.save().await.map_err(session_error)?;
    }
    let sid = current_id(session);
    let now = Utc::now().timestamp();
    let mut slots = load(session, key, quota, now, &sid).await;
    if slots.len() >= quota.per_session && !slots.contains_key(handle) {
        warn!(
            key,
            "session challenge quota reached; refusing new challenge"
        );
        return Err(ErrorResponse::new(ErrorCode::RateLimited)
            .with_message("Too many pending sign-in attempts. Try again shortly."));
    }
    slots.insert(
        handle.to_owned(),
        Slot {
            issued_at: now,
            body,
            session_id: sid,
        },
    );
    session.insert(key, &slots).await.map_err(session_error)?;
    session.save().await.map_err(session_error)
}

/// Removes and returns the body for `handle` while it is live. A miss writes nothing.
pub async fn take(
    session: &Session,
    key: &str,
    quota: Quota,
    handle: &str,
) -> Result<Option<String>, ErrorResponse> {
    let _guard = lock().await;
    let now = Utc::now().timestamp();
    let sid = current_id(session);
    let mut slots = load(session, key, quota, now, &sid).await;
    let Some(slot) = slots.remove(handle) else {
        return Ok(None);
    };
    if slots.is_empty() {
        session.remove::<Slots>(key).await.map_err(session_error)?;
    } else {
        session.insert(key, &slots).await.map_err(session_error)?;
    }
    session.save().await.map_err(session_error)?;
    Ok(Some(slot.body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tower_sessions::MemoryStore;

    const QUOTA: Quota = Quota {
        per_session: 2,
        ttl_secs: 300,
    };

    fn session() -> Session {
        Session::new(None, Arc::new(MemoryStore::default()), None)
    }

    #[tokio::test]
    async fn take_is_single_use() {
        let s = session();
        insert(&s, "k", QUOTA, "h", "body".into()).await.unwrap();
        assert_eq!(
            take(&s, "k", QUOTA, "h").await.unwrap().as_deref(),
            Some("body")
        );
        assert!(take(&s, "k", QUOTA, "h").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn full_session_refuses_instead_of_evicting() {
        let s = session();
        insert(&s, "k", QUOTA, "a", "1".into()).await.unwrap();
        insert(&s, "k", QUOTA, "b", "2".into()).await.unwrap();
        let refused = insert(&s, "k", QUOTA, "c", "3".into()).await.unwrap_err();
        assert_eq!(refused.code, ErrorCode::RateLimited);
        assert_eq!(
            take(&s, "k", QUOTA, "a").await.unwrap().as_deref(),
            Some("1")
        );
        assert_eq!(
            take(&s, "k", QUOTA, "b").await.unwrap().as_deref(),
            Some("2")
        );
    }

    #[tokio::test]
    async fn slot_survives_new_session_handle_on_same_store() {
        let store = Arc::new(MemoryStore::default());
        let first = Session::new(None, store.clone(), None);
        insert(&first, "k", QUOTA, "h", "body".into())
            .await
            .unwrap();
        let id = first.id();
        // A fresh Session object over the same store has no in-memory state of its own.
        let restarted = Session::new(id, store, None);
        assert_eq!(
            take(&restarted, "k", QUOTA, "h").await.unwrap().as_deref(),
            Some("body")
        );
    }

    #[tokio::test]
    async fn slot_from_before_session_rotation_is_refused() {
        let s = session();
        insert(&s, "k", QUOTA, "h", "body".into()).await.unwrap();
        s.cycle_id().await.unwrap();
        s.save().await.unwrap();
        assert!(take(&s, "k", QUOTA, "h").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn legacy_payload_reads_as_absent() {
        let s = session();
        s.insert("k", "legacy-plain-string").await.unwrap();
        s.save().await.unwrap();
        assert!(take(&s, "k", QUOTA, "h").await.unwrap().is_none());
        insert(&s, "k", QUOTA, "h", "fresh".into()).await.unwrap();
        assert_eq!(
            take(&s, "k", QUOTA, "h").await.unwrap().as_deref(),
            Some("fresh")
        );
    }
}
