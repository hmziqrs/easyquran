//! Search page (PARITY E1, E2, E4, E5). The command palette sources (E3)
//! feed `shell::palette`. Contract: `pub fn page() -> impl IntoElement`.

use nsh_core::prelude::*;

pub fn page() -> impl IntoElement {
    super::placeholder("Search", None)
}
