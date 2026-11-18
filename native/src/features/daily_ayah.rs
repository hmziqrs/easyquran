//! Daily ayah (PARITY H2). Contract: `pub fn page() -> impl IntoElement`.

use nsh_core::prelude::*;

pub fn page() -> impl IntoElement {
    super::placeholder("Daily ayah", None)
}
