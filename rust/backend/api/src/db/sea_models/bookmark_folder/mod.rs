mod actions;
mod model;
mod slice;

pub use model::*;
pub use slice::*;

// Shared with the bookmark entity: every LWW write path must normalize
// timestamps through the same canonical form or the TEXT comparisons break.
pub(crate) use actions::lww_timestamp;
