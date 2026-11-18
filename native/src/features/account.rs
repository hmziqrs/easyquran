//! Account page: profile, sessions, devices, delete account (PARITY I7,
//! I9, I3 setup, I4 link, I5). Contract: `pub fn page() -> impl IntoElement`.

use nsh_core::prelude::*;

pub fn page() -> impl IntoElement {
    super::placeholder("Account", None)
}
