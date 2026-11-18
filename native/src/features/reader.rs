//! The reader (PARITY C2-C12, B6): surah, page, juz and hizb readers,
//! translations, scripts, verse tools. Contract:
//! `pub fn page(target: ReaderTarget) -> impl IntoElement`.

use nsh_core::prelude::*;

use crate::shell::routes::ReaderTarget;

pub fn page(target: ReaderTarget) -> impl IntoElement {
    super::placeholder("Reader", Some(format!("{target:?}")))
}
