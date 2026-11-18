//! The forms feature: the sign-up page (every `nsh-form` behavior) and the
//! settings form that loads with `form.load`. Owned by the forms area.
//!
//! Mounting contract for the shell: `forms_page` returns the feature's own
//! `nav_stack` in a view, made to be placed as content (inside the shell's
//! `nav_page` or pane). The feature owns its inner routes, so its pages'
//! `pop_on_back` guards and their confirm dialogs can pop by themselves;
//! a back press reaches them first, as the innermost stack that takes it.
//! Do not wrap it in another `nav_stack` of the shell's.

pub mod fields;
pub mod settings;
pub mod sign_up;

use nsh_core::prelude::*;

use sign_up::sign_up_page;

/// The sign-up page (see the module docs for how the shell mounts it).
pub fn forms_page() -> impl IntoElement {
    sign_up_page()
}
