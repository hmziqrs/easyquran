//! Azkar list and zikr detail (PARITY H1). Contract:
//! `pub fn page() -> impl IntoElement`,
//! `pub fn zikr_page(slug: String, translation: Option<TranslationRef>) -> impl IntoElement`.

use nsh_core::prelude::*;

use crate::shell::routes::TranslationRef;

pub fn page() -> impl IntoElement {
    super::placeholder("Azkar", None)
}

pub fn zikr_page(slug: String, translation: Option<TranslationRef>) -> impl IntoElement {
    super::placeholder("Zikr", Some(format!("{slug} {translation:?}")))
}
