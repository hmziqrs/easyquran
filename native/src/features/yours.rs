//! "Yours" hub and card detail (PARITY G1-G6, F2, F3). Contract:
//! `pub fn page() -> impl IntoElement`,
//! `pub fn card_page(id: String) -> impl IntoElement`.

use nsh_core::prelude::*;

pub fn page() -> impl IntoElement {
    super::placeholder("Yours", None)
}

pub fn card_page(id: String) -> impl IntoElement {
    super::placeholder("Card", Some(id))
}
