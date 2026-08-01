use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

pub use ruxlog_types::enums::PayoutAccountStatus;

use crate::error::{DbResult, ErrorCode, ErrorResponse};
use crate::utils::field_crypto;

const ENC_KEY: &str = "enc";

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "payout_accounts")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub user_id: i32,
    pub provider: String,
    pub provider_account_id: String,
    pub status: PayoutAccountStatus,
    /// Encrypted envelope `{"enc": "..."}`, NOT plaintext metadata; read via [`Model::decrypted_metadata`], writes auto-encrypt in `ActiveModelBehavior`.
    pub metadata: Option<Json>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::super::user::Entity",
        from = "Column::UserId",
        to = "super::super::user::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    User,
}

impl Related<super::super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(mut self, _db: &C, _insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if let sea_orm::ActiveValue::Set(Some(plaintext_json)) = self.metadata.clone() {
            let envelope = encrypt_metadata(&plaintext_json).map_err(|err| {
                DbErr::Custom(format!("payout_account.metadata encryption failed: {err}"))
            })?;
            self.metadata = sea_orm::ActiveValue::Set(Some(envelope));
        }
        Ok(self)
    }
}

pub fn encrypt_metadata(plaintext: &Json) -> Result<Json, field_crypto::FieldCryptoError> {
    if is_encrypted_envelope(plaintext) {
        return Ok(plaintext.clone());
    }
    let plaintext_str =
        serde_json::to_string(plaintext).map_err(|_| field_crypto::FieldCryptoError::Encrypt)?;
    let ciphertext = field_crypto::encrypt(&plaintext_str)?;
    Ok(serde_json::json!({ ENC_KEY: ciphertext }))
}

pub fn is_encrypted_envelope(value: &Json) -> bool {
    value
        .as_object()
        .map(|obj| obj.len() == 1 && obj.get(ENC_KEY).map(|v| v.is_string()).unwrap_or(false))
        .unwrap_or(false)
}

pub fn decrypt_metadata(stored: &Option<Json>) -> DbResult<Option<Json>> {
    let Some(value) = stored else {
        return Ok(None);
    };

    if is_encrypted_envelope(value) {
        let ciphertext = value
            .get(ENC_KEY)
            .and_then(|v| v.as_str())
            .expect("is_encrypted_envelope guarantees a string `enc` value");
        let plaintext_str = field_crypto::decrypt(ciphertext).map_err(|e| {
            ErrorResponse::new(ErrorCode::InternalServerError)
                .with_message("Payout metadata decryption failed")
                .with_details(e.to_string())
        })?;
        let plaintext_json: Json = serde_json::from_str(&plaintext_str)?;
        return Ok(Some(plaintext_json));
    }

    tracing::warn!(
        "payout_account.metadata row is plaintext (pre-V-MED-11) or malformed; \
         returning as-is without decryption. Backfill before treating as safe."
    );
    Ok(Some(value.clone()))
}

impl Model {
    pub fn decrypted_metadata(&self) -> DbResult<Option<Json>> {
        decrypt_metadata(&self.metadata)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> [u8; 32] {
        let mut k = [0u8; 32];
        for (i, b) in k.iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(7).wrapping_add(0x11);
        }
        k
    }

    #[test]
    fn metadata_round_trips_through_envelope() {
        let key = test_key();
        field_crypto::set_key(&key).ok();
        let plaintext = serde_json::json!({
            "iban": "DE89370400440532013000",
            "holder": "Jane Doe",
            "bank_code": "37040044"
        });
        let envelope = encrypt_metadata(&plaintext).expect("encrypt");
        assert!(
            envelope.get(ENC_KEY).and_then(|v| v.as_str()).is_some(),
            "envelope must wrap ciphertext under `{ENC_KEY}`"
        );
        assert!(
            !envelope.to_string().contains("iban"),
            "plaintext field name leaked into ciphertext envelope"
        );
        let recovered = decrypt_metadata(&Some(envelope)).expect("decrypt");
        assert_eq!(recovered, Some(plaintext));
    }

    #[test]
    fn none_metadata_round_trips_to_none() {
        let recovered = decrypt_metadata(&None).expect("none should decrypt to none");
        assert!(recovered.is_none());
    }

    #[test]
    fn tampered_envelope_fails_closed() {
        field_crypto::set_key(&test_key()).ok();
        let plaintext = serde_json::json!({"wallet": "0xabc"});
        let mut envelope = encrypt_metadata(&plaintext).expect("encrypt");
        let ct = envelope
            .get(ENC_KEY)
            .and_then(|v| v.as_str())
            .expect("envelope has enc")
            .to_string();
        let mut chars: Vec<char> = ct.chars().collect();
        let mid = chars.len() / 2;
        chars[mid] = if chars[mid] == 'A' { 'B' } else { 'A' };
        envelope[ENC_KEY] = serde_json::Value::String(chars.into_iter().collect());
        let err = decrypt_metadata(&Some(envelope)).expect_err("tampered must fail closed");
        assert_eq!(err.code, crate::error::ErrorCode::InternalServerError);
        assert_eq!(err.status, 500u16);
    }

    #[test]
    fn legacy_plaintext_row_is_returned_unencrypted_with_warning() {
        let legacy = serde_json::json!({"iban": "DE00", "holder": "Old"});
        let recovered =
            decrypt_metadata(&Some(legacy.clone())).expect("legacy row must not hard-fail");
        assert_eq!(recovered, Some(legacy));
    }

    #[test]
    fn different_plaintexts_produce_different_envelopes() {
        field_crypto::set_key(&test_key()).ok();
        let a = encrypt_metadata(&serde_json::json!({"x": 1})).expect("encrypt");
        let b = encrypt_metadata(&serde_json::json!({"x": 1})).expect("encrypt");
        assert_ne!(
            a.get(ENC_KEY).and_then(|v| v.as_str()),
            b.get(ENC_KEY).and_then(|v| v.as_str()),
            "nonce reuse produced identical ciphertexts"
        );
    }

    #[test]
    fn encrypt_metadata_is_idempotent_on_envelopes() {
        field_crypto::set_key(&test_key()).ok();
        let plaintext = serde_json::json!({"wallet": "0xdeadbeef"});
        let envelope = encrypt_metadata(&plaintext).expect("encrypt");
        let re_encrypted = encrypt_metadata(&envelope).expect("idempotent re-encrypt");
        assert_eq!(
            envelope, re_encrypted,
            "encrypt_metadata must pass an existing envelope through unchanged"
        );
        let recovered = decrypt_metadata(&Some(envelope)).expect("decrypt");
        assert_eq!(recovered, Some(plaintext));
    }

    #[test]
    fn is_encrypted_envelope_detector() {
        assert!(is_encrypted_envelope(&serde_json::json!({"enc": "YWJj"})));
        assert!(!is_encrypted_envelope(
            &serde_json::json!({"enc": "YWJj", "extra": 1})
        ));
        assert!(!is_encrypted_envelope(&serde_json::json!({"iban": "DE00"})));
        assert!(!is_encrypted_envelope(&serde_json::json!({"enc": 42})));
        assert!(!is_encrypted_envelope(&serde_json::json!("plain string")));
    }
}
