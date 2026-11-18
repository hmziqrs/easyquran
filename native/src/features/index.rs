//! Surah, juz and pages indexes (PARITY C1, B3). Contract:
//! `pub fn surah_index_page() -> impl IntoElement`,
//! `pub fn juz_index_page() -> impl IntoElement`,
//! `pub fn pages_index_page() -> impl IntoElement`.

use nsh_core::prelude::*;

pub fn surah_index_page() -> impl IntoElement {
    super::placeholder("Read", None)
}

pub fn juz_index_page() -> impl IntoElement {
    super::placeholder("Juz", None)
}

pub fn pages_index_page() -> impl IntoElement {
    super::placeholder("Pages", None)
}
