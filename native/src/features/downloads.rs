//! Downloads screen and progress (PARITY K1, K2, B5). Contract:
//! `pub fn page() -> impl IntoElement`.

use nsh_core::prelude::*;

pub fn page() -> impl IntoElement {
    super::placeholder("Downloads", None)
}
