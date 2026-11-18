//! The status table of contracts/sync.md, the same on every client (web
//! `lib/sync/round-status.ts`), and the cross-copy `dirty` rule.

use serde_json::{Value, json};

use super::transport::SyncHttpError;
use crate::net::NetError;

/// The wire protocol this build speaks.
pub const SYNC_PROTOCOL: u32 = 1;
const DEFAULT_RETRY_AFTER_S: u64 = 60;

/// `client` in every sync body: in the body, so the CORS table never changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientInfo {
    /// At most 16 chars: `ios`, `android`, `macos`, `windows`, `linux`.
    pub platform: String,
    /// At most 32 chars.
    pub app_version: String,
}

impl ClientInfo {
    pub fn to_json(&self) -> Value {
        json!({
            "protocol": SYNC_PROTOCOL,
            "platform": self.platform.chars().take(16).collect::<String>(),
            "appVersion": self.app_version.chars().take(32).collect::<String>(),
        })
    }
}

impl Default for ClientInfo {
    fn default() -> Self {
        Self {
            platform: "native".into(),
            app_version: env!("CARGO_PKG_VERSION").into(),
        }
    }
}

/// Why a round produced no applicable page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoundFailure {
    /// 401, 403: pause without penalty, keep every dirty row. `account_mismatch`
    /// is the 403 whose cookie account is not the expected one: the auth seam
    /// should re-probe the session.
    Paused { status: u16, account_mismatch: bool },
    /// 426: pause sync, show the update prompt, keep reading.
    UpgradeRequired,
    /// 429: hold this domain's pushes this long; pulls continue.
    RetryAfter { seconds: u64 },
    /// 507: a full pull is over the live quota. Surfaced with its message.
    SnapshotTooLarge { message: String },
    /// Offline, 404, 5xx, malformed 400, unreadable body: back off, rows stay dirty.
    Failed { message: String, offline: bool },
}

impl RoundFailure {
    /// Text for `last_error`.
    pub fn message(&self) -> String {
        match self {
            RoundFailure::Paused { status, .. } => format!("sync paused ({status})"),
            RoundFailure::UpgradeRequired => "sync paused: update required".into(),
            RoundFailure::RetryAfter { seconds } => format!("sync rate limited ({seconds}s)"),
            RoundFailure::SnapshotTooLarge { message } | RoundFailure::Failed { message, .. } => {
                message.clone()
            }
        }
    }
}

/// Maps a failed `SyncPost` call through the status table.
pub fn classify(label: &str, mismatch_message: &str, error: &NetError) -> RoundFailure {
    let Some(http) = SyncHttpError::from_net_error(error) else {
        return match error {
            NetError::Offline(_) => RoundFailure::Failed {
                message: "offline".into(),
                offline: true,
            },
            other => RoundFailure::Failed {
                message: format!("{label} failed: {other}"),
                offline: false,
            },
        };
    };
    match http.status {
        401 => RoundFailure::Paused {
            status: 401,
            account_mismatch: false,
        },
        403 => RoundFailure::Paused {
            status: 403,
            account_mismatch: http.message() == Some(mismatch_message),
        },
        426 => RoundFailure::UpgradeRequired,
        429 => RoundFailure::RetryAfter {
            seconds: http.retry_after.unwrap_or(DEFAULT_RETRY_AFTER_S),
        },
        507 => RoundFailure::SnapshotTooLarge {
            message: format!(
                "{label} failed (507): the library is larger than a full sync can send"
            ),
        },
        status => RoundFailure::Failed {
            message: format!("{label} failed ({status})"),
            offline: false,
        },
    }
}

/// `dirty` after merging two copies of one row. OR across copies, except that
/// an ack is final for the stamps it named: identical stamps clean on either
/// side are clean, and a dirty side whose registers all lost has nothing left
/// to send.
pub fn merged_dirty(
    a: (bool, &[Option<String>]),
    b: (bool, &[Option<String>]),
    merged: &[Option<String>],
) -> bool {
    let (a_dirty, a_stamps) = a;
    let (b_dirty, b_stamps) = b;
    let same = a_stamps
        .iter()
        .enumerate()
        .all(|(i, s)| b_stamps.get(i) == Some(s));
    if same {
        return a_dirty && b_dirty;
    }
    let contributes = |x: &[Option<String>], y: &[Option<String>]| {
        merged
            .iter()
            .enumerate()
            .any(|(i, s)| x.get(i) == Some(s) && y.get(i) != Some(s))
    };
    (a_dirty && contributes(a_stamps, b_stamps)) || (b_dirty && contributes(b_stamps, a_stamps))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failure(status: u16, body: Value) -> RoundFailure {
        let err = SyncHttpError::new(status, None, body).into_net_error();
        classify("bookmarks sync", "Bookmark sync account changed", &err)
    }

    #[test]
    fn maps_every_status_in_the_table() {
        assert_eq!(
            failure(401, Value::Null),
            RoundFailure::Paused {
                status: 401,
                account_mismatch: false
            }
        );
        assert_eq!(
            failure(403, json!({"message": "Bookmark sync account changed"})),
            RoundFailure::Paused {
                status: 403,
                account_mismatch: true
            }
        );
        assert_eq!(failure(426, Value::Null), RoundFailure::UpgradeRequired);
        assert_eq!(
            failure(429, Value::Null),
            RoundFailure::RetryAfter { seconds: 60 }
        );
        assert!(matches!(
            failure(507, Value::Null),
            RoundFailure::SnapshotTooLarge { .. }
        ));
        for status in [400, 404, 500, 503] {
            assert!(matches!(
                failure(status, Value::Null),
                RoundFailure::Failed { offline: false, .. }
            ));
        }
        assert_eq!(
            classify("x", "y", &NetError::Offline("timeout".into())),
            RoundFailure::Failed {
                message: "offline".into(),
                offline: true
            }
        );
    }

    #[test]
    fn dirty_is_or_except_an_ack_is_final_for_its_stamps() {
        let s = |v: &str| Some(v.to_string());
        let one = [s("a")];
        let two = [s("b")];
        // Same stamps: clean on either side wins.
        assert!(!merged_dirty((true, &one), (false, &one), &one));
        assert!(merged_dirty((true, &one), (true, &one), &one));
        // The dirty side's register won: still dirty.
        assert!(merged_dirty((true, &two), (false, &one), &two));
        // The dirty side's register lost: nothing left to send.
        assert!(!merged_dirty((true, &one), (false, &two), &two));
    }
}
