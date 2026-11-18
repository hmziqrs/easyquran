//! Home / landing (PARITY A3): cards for continue, daily ayah, next zikr,
//! custom cards. Contract: `pub fn page() -> impl IntoElement`.

use nsh_core::prelude::*;

pub fn page() -> impl IntoElement {
    super::placeholder("Home", None)
}
