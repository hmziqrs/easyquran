//! Settings shell and sections (PARITY J1-J9, N1). Contract:
//! `pub fn page(section: SettingsSection) -> impl IntoElement`.

use nsh_core::prelude::*;

use crate::shell::routes::SettingsSection;

pub fn page(section: SettingsSection) -> impl IntoElement {
    super::placeholder("Settings", Some(section.slug().to_owned()))
}
