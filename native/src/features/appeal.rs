//! Ban appeal flow (PARITY I10). Contract:
//! `pub fn page(token: Option<String>) -> impl IntoElement`.

use nsh_core::prelude::*;

pub fn page(token: Option<String>) -> impl IntoElement {
    super::placeholder("Appeal", token)
}
