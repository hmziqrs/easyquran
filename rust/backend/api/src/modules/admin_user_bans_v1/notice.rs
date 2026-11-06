//! Enforcement notice delivery (A21). The notice row exists before any send. The
//! stored `delivery_state` is what the provider confirmed, never what we hoped:
//! `sent` needs a provider that accepted the message, and an `Ok` that accepted
//! nothing is `send_failed`.

use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;
use tracing::warn;

use crate::db::sea_models::enforcement_notice::{self, DeliveryState};
use crate::error::DbResult;
use crate::services::mail::{self, EnforcementNoticeMail, MailError, MailRouter, SendReceipt};

/// English and Arabic labels for the closed reason codes (A19). Unknown codes map
/// to "other", so a stored code can never render as raw text in a notice.
pub fn reason_copy(code: &str) -> (&'static str, &'static str) {
    match code {
        "spam" => ("Spam", "رسائل مزعجة"),
        "harassment" => ("Harassment", "مضايقة"),
        "credential_abuse" => ("Credential abuse", "إساءة استخدام بيانات الدخول"),
        "policy_violation" => ("Policy violation", "مخالفة السياسات"),
        _ => ("Other", "أخرى"),
    }
}

/// Maps one send attempt to the stored state. Only a provider that accepted the
/// message (delivered or queued) counts as `sent`.
pub fn delivery_state_for(result: &Result<SendReceipt, MailError>) -> DeliveryState {
    match result {
        Ok(receipt) if receipt.suppressed => DeliveryState::Suppressed,
        Ok(receipt) if receipt.accepted_by_provider() => DeliveryState::Sent,
        Ok(_) => DeliveryState::SendFailed,
        Err(MailError::Suppressed) => DeliveryState::Suppressed,
        Err(MailError::InvalidRecipient(_)) => DeliveryState::NoContactDetails,
        Err(_) => DeliveryState::SendFailed,
    }
}

fn format_expiry(epoch_secs: i64) -> String {
    DateTime::<Utc>::from_timestamp(epoch_secs, 0)
        .map(|at| at.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Sends one notice and records the outcome on its row. `contact` is `None` when
/// the account no longer exists: there is nowhere to send, so no mail is attempted.
pub async fn deliver(
    db: &DatabaseConnection,
    mailer: &MailRouter,
    app_name: &str,
    notice: &enforcement_notice::Model,
    contact: Option<(&str, &str)>,
    appeal_url: Option<&str>,
) -> DbResult<DeliveryState> {
    let (state, sent_at) = match contact {
        None => (DeliveryState::NoContactDetails, None),
        Some((email, user_name)) => {
            let (reason_en, reason_ar) = reason_copy(&notice.reason_code);
            let expires_at = format_expiry(notice.expires_at);
            let result = mail::send_enforcement_notice(
                mailer,
                email,
                &EnforcementNoticeMail {
                    app_name,
                    user_name,
                    reason_en,
                    reason_ar,
                    expires_at: &expires_at,
                    appeal_url,
                    comment: notice.subject_kind == enforcement_notice::SUBJECT_COMMENT,
                },
            )
            .await;
            let state = delivery_state_for(&result);
            if let (DeliveryState::SendFailed, Err(err)) = (state, &result) {
                warn!(
                    notice_id = notice.id,
                    error_kind = mail::mail_error_kind(err),
                    "enforcement notice not delivered"
                );
            }
            let sent_at = if state == DeliveryState::Sent {
                Some(Utc::now().timestamp())
            } else {
                None
            };
            (state, sent_at)
        }
    };
    enforcement_notice::Entity::record_delivery(db, notice.id, state, sent_at).await?;
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_provider_accept_counts_as_sent() {
        let accepted = SendReceipt {
            delivered: 1,
            ..SendReceipt::default()
        };
        let queued = SendReceipt {
            queued: 1,
            ..SendReceipt::default()
        };
        assert_eq!(delivery_state_for(&Ok(accepted)), DeliveryState::Sent);
        assert_eq!(delivery_state_for(&Ok(queued)), DeliveryState::Sent);
        assert_eq!(
            delivery_state_for(&Ok(SendReceipt::default())),
            DeliveryState::SendFailed
        );
    }

    #[test]
    fn suppression_and_contact_errors_map_to_their_states() {
        let suppressed = SendReceipt {
            suppressed: true,
            ..SendReceipt::default()
        };
        assert_eq!(
            delivery_state_for(&Ok(suppressed)),
            DeliveryState::Suppressed
        );
        assert_eq!(
            delivery_state_for(&Err(MailError::Suppressed)),
            DeliveryState::Suppressed
        );
        assert_eq!(
            delivery_state_for(&Err(MailError::InvalidRecipient("x".into()))),
            DeliveryState::NoContactDetails
        );
        assert_eq!(
            delivery_state_for(&Err(MailError::Other("boom".into()))),
            DeliveryState::SendFailed
        );
    }

    #[test]
    fn unknown_reason_code_renders_as_other() {
        assert_eq!(reason_copy("spam").0, "Spam");
        assert_eq!(reason_copy("made_up_code").0, "Other");
    }
}
