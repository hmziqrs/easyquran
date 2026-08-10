pub mod cloudflare;
pub mod error_map;
mod html_templates;
pub mod none;
pub mod provider;
pub mod router;
pub mod smtp;
pub mod templates;

pub use error_map::mail_error_to_response;
pub use provider::{MailError, MailProvider, OutboundEmail, SendReceipt};
pub use router::MailRouter;

use provider::{TEMPLATE_PASSWORD_RESET, TEMPLATE_VERIFICATION};

pub async fn send_email_verification_code(
    mailer: &MailRouter,
    email: &str,
    code: &str,
) -> Result<(), MailError> {
    let body = html_templates::email_otp_html(code);
    mailer
        .send(OutboundEmail {
            to: email.to_string(),
            subject: "Email verification code".to_string(),
            html: Some(body),
            text: None,
            template: Some(TEMPLATE_VERIFICATION),
        })
        .await?;
    Ok(())
}

pub async fn send_forgot_password_email(
    mailer: &MailRouter,
    email: &str,
    code: &str,
) -> Result<(), MailError> {
    let body = html_templates::email_otp_html(code);
    mailer
        .send(OutboundEmail {
            to: email.to_string(),
            subject: "Password reset verification code".to_string(),
            html: Some(body),
            text: None,
            template: Some(TEMPLATE_PASSWORD_RESET),
        })
        .await?;
    Ok(())
}
