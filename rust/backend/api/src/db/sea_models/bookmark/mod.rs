mod actions;
mod model;
mod slice;

pub use actions::{
    registers_of, BookmarkOutcome, BookmarkWrite, BOOKMARKS_DOMAIN, LIVE_BOOKMARK_QUOTA,
};
pub use model::*;
pub use slice::*;
