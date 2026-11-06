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

/// Non-PII discriminant for a [`MailError`]: names the failure class only
/// (never the recipient address / server response that variants like
/// `ProviderApi` or `InvalidRecipient` can carry). Use this in tracing logs
/// instead of the full `Display`, which may echo PII from upstream SMTP/API.
pub fn mail_error_kind(err: &MailError) -> &'static str {
    match err {
        MailError::Config(_) => "config",
        MailError::ProviderApi(_) => "provider_api",
        MailError::Request(_) => "request",
        MailError::Throttled { .. } => "throttled",
        MailError::LimiterUnavailable => "limiter_unavailable",
        MailError::Suppressed => "suppressed",
        MailError::InvalidRecipient(_) => "invalid_recipient",
        MailError::WebhookVerification(_) => "webhook_verification",
        MailError::Other(_) => "other",
    }
}

use provider::{TEMPLATE_PASSWORD_RESET, TEMPLATE_VERIFICATION};

/// Fields of an account-enforcement notice (A21). Copy is bilingual (en and ar)
/// inside the template. `appeal_url` is set only once an appeal page exists (A35).
pub struct EnforcementNoticeMail<'a> {
    pub app_name: &'a str,
    pub user_name: &'a str,
    pub reason_en: &'a str,
    pub reason_ar: &'a str,
    pub expires_at: &'a str,
    pub appeal_url: Option<&'a str>,
}

/// Sends the notice through the router. The returned receipt is the only proof of
/// delivery: callers record `sent` only when `accepted_by_provider()` holds.
pub async fn send_enforcement_notice(
    mailer: &MailRouter,
    email: &str,
    notice: &EnforcementNoticeMail<'_>,
) -> Result<SendReceipt, MailError> {
    let mut ctx = tera::Context::new();
    ctx.insert("app_name", notice.app_name);
    ctx.insert("user_name", notice.user_name);
    ctx.insert("reason_en", notice.reason_en);
    ctx.insert("reason_ar", notice.reason_ar);
    ctx.insert("expires_at", notice.expires_at);
    ctx.insert("appeal_url", &notice.appeal_url);
    let html = templates::render("enforcement_notice", &ctx)
        .map_err(|e| MailError::Other(format!("enforcement notice template: {e}")))?;
    mailer
        .send(OutboundEmail {
            to: email.to_string(),
            subject: "Notice about your account / إشعار بخصوص حسابك".to_string(),
            html: Some(html),
            text: None,
            template: Some(provider::TEMPLATE_ENFORCEMENT_NOTICE),
        })
        .await
}

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
