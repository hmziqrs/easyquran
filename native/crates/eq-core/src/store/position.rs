//! Where a domain's pull stands, saved with the rows it covers so an
//! interrupted pull resumes (contracts/sync.md "Pull").

use rusqlite::Connection;

use super::{
    StoreError,
    rows::{kv_get, kv_remove, kv_set},
};
use crate::sync::SyncDomain;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Position {
    /// Last `rev` applied; 0 is a full pull.
    pub since: i64,
    /// Page cursor inside a full pull; -1 to start.
    pub after: i64,
    /// The server's `syncEpoch` last seen; a change means a restore.
    pub epoch: Option<String>,
}

impl Default for Position {
    fn default() -> Self {
        Self {
            since: 0,
            after: -1,
            epoch: None,
        }
    }
}

fn key(domain: SyncDomain, field: &str) -> String {
    format!("sync.{}.{field}", domain.key())
}

impl Position {
    pub(crate) fn load(conn: &Connection, domain: SyncDomain) -> Result<Self, StoreError> {
        let num = |field: &str, fallback: i64| -> Result<i64, StoreError> {
            Ok(kv_get(conn, &key(domain, field))?
                .and_then(|v| v.parse().ok())
                .unwrap_or(fallback))
        };
        Ok(Self {
            since: num("since", 0)?.max(0),
            after: num("after", -1)?.max(-1),
            epoch: kv_get(conn, &key(domain, "epoch"))?.filter(|e| !e.is_empty()),
        })
    }

    pub(crate) fn save(&self, conn: &Connection, domain: SyncDomain) -> Result<(), StoreError> {
        kv_set(conn, &key(domain, "since"), &self.since.to_string())?;
        kv_set(conn, &key(domain, "after"), &self.after.to_string())?;
        match &self.epoch {
            Some(epoch) => kv_set(conn, &key(domain, "epoch"), epoch),
            None => kv_remove(conn, &key(domain, "epoch")),
        }
    }
}

/// The page fields every sync response shares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PageMeta {
    pub server_time: Option<i64>,
    pub sync_epoch: Option<String>,
    pub rev: i64,
    pub latest_rev: i64,
    pub more: bool,
    pub tombstone_floor: i64,
}

impl PageMeta {
    pub(crate) fn decode(round: &serde_json::Value) -> Self {
        let num = |k: &str| round.get(k).and_then(serde_json::Value::as_i64);
        Self {
            server_time: num("serverTime"),
            sync_epoch: round
                .get("syncEpoch")
                .and_then(serde_json::Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_owned),
            rev: num("rev").unwrap_or(0).max(0),
            latest_rev: num("latestRev").unwrap_or(0).max(0),
            more: round.get("more").and_then(serde_json::Value::as_bool) == Some(true),
            tombstone_floor: num("tombstoneFloor").unwrap_or(0).max(0),
        }
    }
}

/// What a page means for the pull position (web `applyRound`, shared by the
/// row domains).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PageVerdict {
    /// `syncEpoch` changed or `since` is past `latestRev`: the server was
    /// restored. Full pull and re-push everything.
    pub restored: bool,
    /// `since` is below the tombstone floor: full pull, then drop clean rows it did not name.
    pub below_floor: bool,
}

impl PageVerdict {
    pub(crate) fn of(position: &Position, meta: &PageMeta) -> Self {
        let epoch_changed = position.epoch.is_some() && meta.sync_epoch != position.epoch;
        let ahead = position.since > meta.latest_rev;
        Self {
            restored: epoch_changed || ahead,
            below_floor: position.since > 0 && position.since < meta.tombstone_floor,
        }
    }

    pub(crate) fn reset(self) -> bool {
        self.restored || self.below_floor
    }

    /// The position after this page landed.
    pub(crate) fn next(self, position: &Position, meta: &PageMeta) -> Position {
        let epoch = meta.sync_epoch.clone().or_else(|| position.epoch.clone());
        if self.reset() {
            return Position {
                since: 0,
                after: -1,
                epoch,
            };
        }
        if meta.more && position.since == 0 {
            return Position {
                since: 0,
                after: meta.rev,
                epoch,
            };
        }
        Position {
            since: meta.rev,
            after: -1,
            epoch,
        }
    }
}
