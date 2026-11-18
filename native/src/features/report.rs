//! Report content flow (PARITY I11). Contract:
//! `pub fn page() -> impl IntoElement`.

use nsh_core::prelude::*;

pub fn page() -> impl IntoElement {
    super::placeholder("Report", None)
}
