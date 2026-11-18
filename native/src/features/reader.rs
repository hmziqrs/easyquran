//! The reader (PARITY C2-C12, B6): surah, page, juz and hizb readers,
//! translations, scripts, verse tools. Contract:
//! `pub fn page(link: ReaderLink) -> impl IntoElement` (`link.target`, plus
//! the URL's `?mode=` and `?more=`).

use nsh_core::prelude::*;

use crate::shell::routes::ReaderLink;

pub fn page(link: ReaderLink) -> impl IntoElement {
    super::placeholder("Reader", Some(format!("{link:?}")))
}
