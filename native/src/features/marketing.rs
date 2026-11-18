//! Static pages (PARITY A4, A5): about, FAQ, contact, privacy, terms.
//! Contract: `pub fn about_page()`, `faq_page()`, `contact_page()`,
//! `privacy_page()`, `terms_page()`, each `-> impl IntoElement`.

use nsh_core::prelude::*;

pub fn about_page() -> impl IntoElement {
    super::placeholder("About", None)
}

pub fn faq_page() -> impl IntoElement {
    super::placeholder("FAQ", None)
}

pub fn contact_page() -> impl IntoElement {
    super::placeholder("Contact", None)
}

pub fn privacy_page() -> impl IntoElement {
    super::placeholder("Privacy", None)
}

pub fn terms_page() -> impl IntoElement {
    super::placeholder("Terms", None)
}
