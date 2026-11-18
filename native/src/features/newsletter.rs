//! Newsletter confirm (PARITY I12; the signup form sits in the footer).
//! Contract: `pub fn confirm_page(email: String, token: String) -> impl IntoElement`.

use nsh_core::prelude::*;

pub fn confirm_page(email: String, token: String) -> impl IntoElement {
    let _ = email;
    super::placeholder("Newsletter", Some(token))
}
