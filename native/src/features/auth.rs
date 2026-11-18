//! Auth flows (PARITY I2, I3, I4, I13, I14). Contract:
//! `pub fn login_page() -> impl IntoElement`,
//! `pub fn register_page() -> impl IntoElement`,
//! `pub fn verify_email_page() -> impl IntoElement`,
//! `pub fn forgot_password_page() -> impl IntoElement`,
//! `pub fn totp_page() -> impl IntoElement`.

use nsh_core::prelude::*;

pub fn login_page() -> impl IntoElement {
    super::placeholder("Log in", None)
}

pub fn register_page() -> impl IntoElement {
    super::placeholder("Create account", None)
}

pub fn verify_email_page() -> impl IntoElement {
    super::placeholder("Verify email", None)
}

pub fn forgot_password_page() -> impl IntoElement {
    super::placeholder("Forgot password", None)
}

pub fn totp_page() -> impl IntoElement {
    super::placeholder("Two-factor code", None)
}
