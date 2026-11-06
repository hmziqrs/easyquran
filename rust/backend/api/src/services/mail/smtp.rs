use std::env;

use async_trait::async_trait;
use lettre::message::header::{ContentType, Header, HeaderName, HeaderValue};
use lettre::{
    transport::smtp::authentication::Credentials, AsyncSmtpTransport, AsyncTransport, Message,
    Tokio1Executor,
};
use tracing::instrument;

use super::provider::{MailError, MailProvider, OutboundEmail, SendReceipt, LIST_UNSUBSCRIBE_POST};

const TLS_MODE_IMPLICIT: &str = "tls";

/// `List-Unsubscribe` (RFC 2369) with the angle-bracketed URL form.
#[derive(Debug, Clone)]
struct ListUnsubscribe(String);

impl Header for ListUnsubscribe {
    fn name() -> HeaderName {
        HeaderName::new_from_ascii_str("List-Unsubscribe")
    }

    fn parse(s: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self(s.to_string()))
    }

    fn display(&self) -> HeaderValue {
        HeaderValue::new(Self::name(), format!("<{}>", self.0))
    }
}

/// `List-Unsubscribe-Post` (RFC 8058) one-click marker.
#[derive(Debug, Clone)]
struct ListUnsubscribePost;

impl Header for ListUnsubscribePost {
    fn name() -> HeaderName {
        HeaderName::new_from_ascii_str("List-Unsubscribe-Post")
    }

    fn parse(_s: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self)
    }

    fn display(&self) -> HeaderValue {
        HeaderValue::new(Self::name(), LIST_UNSUBSCRIBE_POST.to_string())
    }
}

fn use_implicit_tls() -> bool {
    if let Ok(mode) = env::var("SMTP_TLS_MODE") {
        if mode.eq_ignore_ascii_case(TLS_MODE_IMPLICIT) {
            return true;
        }
    }
    matches!(env::var("SMTP_PORT").ok().as_deref(), Some("465"))
}

#[instrument(name = "smtp_connection_init")]
pub async fn create_connection() -> AsyncSmtpTransport<Tokio1Executor> {
    let host = env::var("SMTP_HOST").expect("SMTP_HOST must be set");
    let username = env::var("SMTP_USERNAME").expect("SMTP_USERNAME must be set");
    let password = env::var("SMTP_PASSWORD").expect("SMTP_PASSWORD must be set");

    let implicit_tls = use_implicit_tls();
    tracing::info!(
        smtp_host = %host,
        tls_mode = if implicit_tls { "implicit(tls/465)" } else { "starttls(587)" },
        "Initializing SMTP transport"
    );

    let creds = Credentials::new(username, password);

    let transport = if implicit_tls {
        AsyncSmtpTransport::<Tokio1Executor>::relay(&host)
            .expect("failed to build implicit-TLS SMTP transport")
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&host)
            .expect("failed to build STARTTLS SMTP transport")
    }
    .credentials(creds)
    .build();

    tracing::info!("SMTP transport built");
    transport
}

pub struct SmtpMailProvider {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from_address: String,
    from_name: Option<String>,
}

impl SmtpMailProvider {
    pub fn new(
        transport: AsyncSmtpTransport<Tokio1Executor>,
        from_address: String,
        from_name: Option<String>,
    ) -> Self {
        Self {
            transport,
            from_address,
            from_name,
        }
    }

    fn sender_header(&self) -> String {
        match &self.from_name {
            Some(name) if !name.trim().is_empty() => {
                format!("{name} <{}>", self.from_address)
            }
            _ => self.from_address.clone(),
        }
    }
}

#[async_trait]
impl MailProvider for SmtpMailProvider {
    fn provider_name(&self) -> &'static str {
        "smtp"
    }

    async fn send(&self, msg: OutboundEmail) -> Result<SendReceipt, MailError> {
        let from = self.sender_header().parse().map_err(|e| {
            MailError::Config(format!(
                "invalid sender address '{value}': {e}",
                value = self.from_address
            ))
        })?;
        let to = msg
            .to
            .parse()
            .map_err(|_| MailError::InvalidRecipient("invalid recipient address".to_string()))?;

        let (content_type, body) = match (&msg.html, &msg.text) {
            (Some(html), _) => (ContentType::TEXT_HTML, html.clone()),
            (None, Some(text)) => (ContentType::TEXT_PLAIN, text.clone()),
            (None, None) => (ContentType::TEXT_PLAIN, String::new()),
        };

        let mut builder = Message::builder()
            .from(from)
            .to(to)
            .subject(msg.subject.as_str());
        if let Some(url) = &msg.list_unsubscribe {
            builder = builder
                .header(ListUnsubscribe(url.clone()))
                .header(ListUnsubscribePost);
        }
        let email = builder
            .header(content_type)
            .body(body)
            .map_err(|e| MailError::ProviderApi(format!("failed to build message: {e}")))?;

        self.transport
            .send(email)
            .await
            .map_err(|e| MailError::ProviderApi(e.to_string()))?;

        Ok(SendReceipt {
            delivered: 1,
            queued: 0,
            permanent_bounces: Vec::new(),
            suppressed: false,
        })
    }
}
