//! Sync engine (PARITY L1, L2): UUID v7 ids, the hybrid clock and stamps,
//! rows merged by `eq-sync-core` (re-exported as [`crate::merge`]), "the row
//! is the queue", pull/push rounds per domain, round status. The store
//! ([`crate::store`]) holds the rows; this module drives rounds over them.

pub mod clock;
pub mod engine;
pub mod ids;
pub mod round_status;
pub mod transport;

use serde_json::Value;

/// A synced domain: one endpoint, one pull position, one hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SyncDomain {
    Bookmarks,
    Sessions,
    Preferences,
}

impl SyncDomain {
    /// Every domain, in round order.
    pub const ALL: [SyncDomain; 3] = [
        SyncDomain::Bookmarks,
        SyncDomain::Sessions,
        SyncDomain::Preferences,
    ];

    /// Storage key segment.
    pub fn key(self) -> &'static str {
        match self {
            SyncDomain::Bookmarks => "bookmarks",
            SyncDomain::Sessions => "sessions",
            SyncDomain::Preferences => "preferences",
        }
    }

    /// The API path a round POSTs to.
    pub fn path(self) -> &'static str {
        match self {
            SyncDomain::Bookmarks => crate::store::bookmarks::SYNC_PATH,
            SyncDomain::Sessions => crate::store::sessions::SYNC_PATH,
            SyncDomain::Preferences => crate::store::preferences::SYNC_PATH,
        }
    }

    /// Name used in failure messages.
    pub fn label(self) -> &'static str {
        match self {
            SyncDomain::Bookmarks => "bookmarks sync",
            SyncDomain::Sessions => "reading-session sync",
            SyncDomain::Preferences => "preference sync",
        }
    }

    /// The server's 403 message when the session's account is not the expected one.
    pub fn mismatch_message(self) -> &'static str {
        match self {
            SyncDomain::Bookmarks => crate::store::bookmarks::MISMATCH_MESSAGE,
            SyncDomain::Sessions => crate::store::sessions::MISMATCH_MESSAGE,
            SyncDomain::Preferences => crate::store::preferences::MISMATCH_MESSAGE,
        }
    }
}

/// What applying one page did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RoundApplied {
    /// More dirty rows or pull pages wait: run another round now.
    pub more: bool,
    /// `settings.*` values that arrived from another device, for the app's
    /// settings store to show (the store applies goals and the home layout itself).
    pub settings: Vec<(String, Value)>,
}
