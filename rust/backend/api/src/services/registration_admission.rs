//! Registration admission (A9).
//!
//! - One daily creation counter per client IP, shared by the password door and
//!   all five OAuth doors, plus one global daily ceiling. A refused creation is a
//!   429 with Retry-After to the next UTC midnight. Counters only: nothing here
//!   bans, and nothing feeds escalation.
//! - A daily per-IP cap on new anonymous CSRF sessions, so the session store
//!   cannot be filled from one address.
//! - A static disposable-domain list for password registration.
//! - First-24-hours limits for new accounts: at most one comment, no links.
//!
//! Counts live in the shared gate store (process-local, flushed to L2 like the
//! other fixed windows). The app runs as a single instance, so one store is the
//! whole count. Cap values come from `config::settings::admission()`.

use std::collections::HashSet;
use std::sync::LazyLock;
use std::time::Duration;

use chrono::{DateTime, Utc};
use rux_request_gate::{InMemoryStore, RateLimitStore};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter};

use crate::db::sea_models::{post_comment, user};
use crate::error::{ErrorCode, ErrorResponse};
use crate::services::write_ceiling::secs_until_midnight_utc;

/// How long an account counts as new: a fixed 24-hour window from `created_at`.
pub const FRESH_ACCOUNT_SECS: i64 = 86_400;

/// Gate-store key for the global daily creation counter.
pub const CREATION_GLOBAL_KEY: &str = "create:global";

const DISPOSABLE_DOMAINS: &str = include_str!("disposable_domains.txt");

/// Caps for one creation attempt, read from `config::settings::admission()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CreationCaps {
    pub per_ip_daily: u64,
    pub global_daily: u64,
}

impl CreationCaps {
    pub fn configured() -> Self {
        let admission = crate::config::settings::admission();
        Self {
            per_ip_daily: admission.registrations_per_ip_daily,
            global_daily: admission.registrations_global_daily,
        }
    }
}

/// Where a creation comes from: the shared gate store and the client IP that
/// identifies the caller. OAuth doors take this so they charge the same counters
/// as the password door.
#[derive(Clone, Copy)]
pub struct CreationSite<'a> {
    pub store: &'a InMemoryStore,
    pub ip: &'a str,
}

/// Gate-store key for one client IP's daily creation counter.
pub fn creation_ip_key(ip: &str) -> String {
    format!("create:ip:{ip}")
}

/// Gate-store key for one client IP's daily anonymous-session counter.
pub fn anonymous_session_ip_key(ip: &str) -> String {
    format!("csrf-session:ip:{ip}")
}

/// The 429 every admission refusal returns. Generic wording: the message does not
/// reveal which cap was hit or whether the address already has an account.
fn refused(retry_after: u64) -> ErrorResponse {
    ErrorResponse::new(ErrorCode::RateLimited)
        .with_message(format!(
            "Too many requests. Try again in {retry_after} seconds."
        ))
        .with_retry_after(retry_after)
}

fn store_unavailable() -> ErrorResponse {
    ErrorResponse::new(ErrorCode::ServiceUnavailable)
        .with_message("Limiter unavailable (store error)")
}

/// Counts one unit against a daily fixed window and refuses past `cap`. The
/// count moves on every attempt, refused ones included, so a burst cannot slip
/// past the cap by racing refusals.
async fn charge_daily(
    store: &InMemoryStore,
    key: &str,
    cap: u64,
    now: DateTime<Utc>,
) -> Result<(), ErrorResponse> {
    let window = Duration::from_secs(secs_until_midnight_utc(now));
    let (count, ttl) = store
        .incr_expire(key, window)
        .await
        .map_err(|_| store_unavailable())?;
    if count <= cap {
        return Ok(());
    }
    Err(refused(ttl.max(1)))
}

/// Admits one new account from `ip`: the per-IP daily count first, then the
/// global daily ceiling. Shared by every creation door, so an OAuth burst and a
/// password burst draw on the same counters.
pub async fn admit_creation(
    store: &InMemoryStore,
    ip: &str,
    caps: CreationCaps,
    now: DateTime<Utc>,
) -> Result<(), ErrorResponse> {
    charge_daily(store, &creation_ip_key(ip), caps.per_ip_daily, now).await?;
    charge_daily(store, CREATION_GLOBAL_KEY, caps.global_daily, now).await
}

/// Admits one new anonymous CSRF session from `ip`. Call only when the request
/// is about to create a session; a request that already carries one is free.
pub async fn admit_anonymous_session(
    store: &InMemoryStore,
    ip: &str,
    cap: u64,
    now: DateTime<Utc>,
) -> Result<(), ErrorResponse> {
    charge_daily(store, &anonymous_session_ip_key(ip), cap, now).await
}

/// Accounts one verification identity (canonical mailbox) may hold while still
/// unverified (A10). A fourth pending registration is refused.
pub const IDENTITY_PENDING_CAP: u64 = 3;

/// Accounts one verification identity may hold over its lifetime (A10). The
/// sixth account is refused, verified or not.
pub const IDENTITY_LIFETIME_CAP: u64 = 5;

/// Refuses a new or re-addressed account when its verification identity already
/// holds the pending or the lifetime cap. `exclude` is the account being
/// re-addressed so it never counts against itself. The refusal is a generic 429:
/// it does not say which cap was reached. Runs inside the caller's transaction,
/// so the count and the write see the same rows.
pub async fn ensure_identity_capacity<C: ConnectionTrait>(
    db: &C,
    identity_hash: &str,
    exclude: Option<i32>,
) -> Result<(), ErrorResponse> {
    let pending = count_identity(db, identity_hash, exclude, true).await?;
    let lifetime = count_identity(db, identity_hash, exclude, false).await?;
    if pending < IDENTITY_PENDING_CAP && lifetime < IDENTITY_LIFETIME_CAP {
        return Ok(());
    }
    Err(ErrorResponse::new(ErrorCode::RateLimited)
        .with_message("Too many requests. Try again later."))
}

async fn count_identity<C: ConnectionTrait>(
    db: &C,
    identity_hash: &str,
    exclude: Option<i32>,
    pending_only: bool,
) -> Result<u64, ErrorResponse> {
    let mut query =
        user::Entity::find().filter(user::Column::VerificationIdentityHash.eq(identity_hash));
    if let Some(id) = exclude {
        query = query.filter(user::Column::Id.ne(id));
    }
    if pending_only {
        query = query.filter(user::Column::IsVerified.eq(false));
    }
    Ok(query.count(db).await?)
}

fn disposable_set() -> &'static HashSet<&'static str> {
    static SET: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
        DISPOSABLE_DOMAINS
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect()
    });
    &SET
}

/// True when the address's domain, or any parent domain of it, is listed.
pub fn is_disposable_email(email: &str) -> bool {
    let Some((_, domain)) = email.rsplit_once('@') else {
        return false;
    };
    let domain = domain.trim().trim_end_matches('.').to_ascii_lowercase();
    let set = disposable_set();
    let mut rest: &str = &domain;
    loop {
        if set.contains(rest) {
            return true;
        }
        match rest.split_once('.') {
            Some((_, parent)) => rest = parent,
            None => return false,
        }
    }
}

/// Whether an account created at `created_at` is still in its first 24 hours.
pub fn is_fresh_account(created_at: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    now.signed_duration_since(created_at).num_seconds() < FRESH_ACCOUNT_SECS
}

/// Conservative link detection for new-account content. Flags a URL scheme,
/// a `www.` prefix, or a bare host with a dotted alphabetic TLD. Errs toward
/// refusing for 24 hours; the cost is a false positive on a dotted word.
pub fn contains_link(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    if lower.contains("://") || lower.contains("www.") || lower.contains("mailto:") {
        return true;
    }
    lower.split_whitespace().any(looks_like_bare_host)
}

fn looks_like_bare_host(token: &str) -> bool {
    let trimmed = token.trim_matches(|c: char| !c.is_ascii_alphanumeric());
    let host = trimmed.split('/').next().unwrap_or("");
    let host = host.split(':').next().unwrap_or("");
    let labels: Vec<&str> = host.split('.').collect();
    if labels.len() < 2 {
        return false;
    }
    let tld = labels[labels.len() - 1];
    let tld_ok = (2..=24).contains(&tld.len()) && tld.chars().all(|c| c.is_ascii_alphabetic());
    let labels_ok = labels.iter().all(|label| {
        !label.is_empty() && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    });
    tld_ok && labels_ok
}

/// Checks a comment write by a fresh account. A link is refused for the whole
/// window. A second comment is refused, so a fresh account gets one comment in
/// its first 24 hours. Runs inside the caller's transaction.
pub async fn check_fresh_comment<C: ConnectionTrait>(
    db: &C,
    author: &user::Model,
    content: &str,
    now: DateTime<Utc>,
    is_create: bool,
) -> Result<(), ErrorResponse> {
    if !is_fresh_account(author.created_at.with_timezone(&Utc), now) {
        return Ok(());
    }
    if contains_link(content) {
        return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
            .with_message("New accounts cannot include links for their first 24 hours"));
    }
    if is_create {
        let existing = post_comment::Entity::find()
            .filter(post_comment::Column::UserId.eq(author.id))
            .count(db)
            .await?;
        if existing >= 1 {
            return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
                .with_message("New accounts can post one comment in their first 24 hours"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(day: u32, hour: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, day, hour, 0, 0).unwrap()
    }

    #[test]
    fn disposable_matches_domain_and_subdomains_only() {
        assert!(is_disposable_email("someone@mailinator.com"));
        assert!(is_disposable_email("someone@Mail.YOPMAIL.com"));
        assert!(!is_disposable_email("someone@gmail.com"));
        assert!(!is_disposable_email("someone@notmailinator.com"));
        assert!(!is_disposable_email("no-at-sign"));
    }

    #[test]
    fn list_has_no_comment_or_blank_entries_leaking_in() {
        let set = disposable_set();
        assert!(set.len() >= 10);
        assert!(set.iter().all(|d| !d.is_empty() && !d.starts_with('#')));
    }

    #[test]
    fn fresh_window_is_24_hours() {
        let created = at(6, 0);
        assert!(is_fresh_account(created, at(6, 23)));
        assert!(!is_fresh_account(created, at(7, 0)));
    }

    #[test]
    fn link_detection_flags_urls_and_bare_hosts() {
        assert!(contains_link("see https://example.com/x"));
        assert!(contains_link("go to www.example.org"));
        assert!(contains_link("visit example.com today"));
        assert!(contains_link("(bit.ly/abc)"));
        assert!(!contains_link("a plain comment about the surah"));
        assert!(!contains_link("e.g. this"));
        assert!(contains_link("ends with example.com."));
    }

    #[tokio::test]
    async fn creation_cap_refuses_past_per_ip_cap() {
        let store = InMemoryStore::default();
        let caps = CreationCaps {
            per_ip_daily: 3,
            global_daily: 100,
        };
        for _ in 0..3 {
            admit_creation(&store, "203.0.113.9", caps, at(6, 12))
                .await
                .unwrap();
        }
        let err = admit_creation(&store, "203.0.113.9", caps, at(6, 12))
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::RateLimited);
        assert!(err.retry_after.is_some_and(|s| s >= 1));
        // Another address is unaffected by this one's count.
        admit_creation(&store, "198.51.100.4", caps, at(6, 12))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn global_ceiling_refuses_across_addresses() {
        let store = InMemoryStore::default();
        let caps = CreationCaps {
            per_ip_daily: 50,
            global_daily: 4,
        };
        for n in 0..4 {
            admit_creation(&store, &format!("192.0.2.{n}"), caps, at(6, 12))
                .await
                .unwrap();
        }
        let err = admit_creation(&store, "192.0.2.200", caps, at(6, 12))
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::RateLimited);
    }

    #[tokio::test]
    async fn concurrent_burst_admits_exactly_the_cap() {
        let store = std::sync::Arc::new(InMemoryStore::default());
        let caps = CreationCaps {
            per_ip_daily: 7,
            global_daily: 1_000,
        };
        let mut tasks = Vec::new();
        for _ in 0..40 {
            let store = store.clone();
            tasks.push(tokio::spawn(async move {
                admit_creation(&store, "203.0.113.50", caps, at(6, 12))
                    .await
                    .is_ok()
            }));
        }
        let mut admitted = 0;
        for task in tasks {
            if task.await.unwrap() {
                admitted += 1;
            }
        }
        assert_eq!(admitted, 7);
    }

    #[tokio::test]
    async fn anonymous_session_cap_refuses_next_session() {
        let store = InMemoryStore::default();
        for _ in 0..2 {
            admit_anonymous_session(&store, "203.0.113.77", 2, at(6, 9))
                .await
                .unwrap();
        }
        let err = admit_anonymous_session(&store, "203.0.113.77", 2, at(6, 9))
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::RateLimited);
    }

    #[test]
    fn creation_keys_are_per_ip_and_global() {
        assert_eq!(creation_ip_key("203.0.113.1"), "create:ip:203.0.113.1");
        assert_ne!(creation_ip_key("a"), CREATION_GLOBAL_KEY);
    }
}
