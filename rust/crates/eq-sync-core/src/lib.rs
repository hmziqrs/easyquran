//! Pure merge rules for every synced domain: no IO, no clock, no database.
//! `contracts/sync.md` is the spec; `contracts/sync-vectors.json` pins this crate
//! and `web/src/lib/sync/merge.ts` to the same results. Rows are JSON objects so
//! a register this build does not know about survives a merge untouched.

mod canonical;
mod merge;
mod stamp;

pub use canonical::canonical_json;
pub use merge::{
    converge_bookmarks, merge_bookmark, merge_event, merge_folder, merge_layout, merge_progress,
    merge_register, merge_rows, merge_settings, merge_thread, round_ratio, Domain, Rule,
};
pub use stamp::{compare_stamps, format_stamp, is_valid_stamp, parse_stamp, ParsedStamp};
