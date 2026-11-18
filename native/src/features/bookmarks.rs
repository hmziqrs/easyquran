//! Bookmarks, folders, sync indicator (PARITY F1, F4). Contract:
//! `pub fn page() -> impl IntoElement`.

use nsh_core::prelude::*;

pub fn page() -> impl IntoElement {
    super::placeholder("Bookmarks", None)
}
