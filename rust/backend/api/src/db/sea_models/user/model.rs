use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

pub use ruxlog_types::enums::UserRole;

use crate::utils::field_crypto;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub name: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub password: Option<String>,
    pub avatar_id: Option<i32>,
    pub is_verified: bool,
    pub role: UserRole,
    pub two_fa_enabled: bool,
    #[serde(skip_serializing)]
    pub two_fa_secret: Option<String>,
    #[serde(skip_serializing)]
    pub two_fa_backup_codes: Option<Json>,
    #[serde(skip_serializing)]
    pub two_fa_last_totp_counter: Option<i64>,
    #[serde(skip_serializing)]
    pub google_id: Option<String>,
    pub oauth_provider: Option<String>,
    #[serde(skip_serializing)]
    pub session_auth_secret: String,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

impl Model {
    pub fn get_role(&self) -> UserRole {
        self.role
    }

    pub fn is_user(&self) -> bool {
        self.get_role().to_i32() >= UserRole::User.to_i32()
    }

    pub fn is_author(&self) -> bool {
        self.get_role().to_i32() >= UserRole::Author.to_i32()
    }

    pub fn is_moderator(&self) -> bool {
        self.get_role().to_i32() >= UserRole::Moderator.to_i32()
    }

    pub fn is_admin(&self) -> bool {
        self.get_role().to_i32() >= UserRole::Admin.to_i32()
    }

    pub fn is_super_admin(&self) -> bool {
        self.get_role().to_i32() >= UserRole::SuperAdmin.to_i32()
    }

    pub fn two_fa_secret_plain(&self) -> Result<Option<String>, field_crypto::FieldCryptoError> {
        self.two_fa_secret
            .as_deref()
            .map(decrypt_two_fa_secret_value)
            .transpose()
    }

    pub fn google_id_plain(&self) -> Result<Option<String>, field_crypto::FieldCryptoError> {
        self.google_id
            .as_deref()
            .map(decrypt_google_id_value)
            .transpose()
    }
}

pub fn decrypt_two_fa_secret_value(blob: &str) -> Result<String, field_crypto::FieldCryptoError> {
    if looks_like_envelope(blob) {
        field_crypto::decrypt(blob)
    } else {
        Ok(blob.to_string())
    }
}

pub fn decrypt_google_id_value(blob: &str) -> Result<String, field_crypto::FieldCryptoError> {
    if looks_like_envelope(blob) {
        field_crypto::decrypt_deterministic(blob)
    } else {
        Ok(blob.to_string())
    }
}

pub fn looks_like_envelope(blob: &str) -> bool {
    let Some(rest) = blob.strip_prefix(|c: char| c == 'V' || c == 'D') else {
        return false;
    };
    let Some(colon) = rest.find(':') else {
        return false;
    };
    let ver = &rest[..colon];
    !ver.is_empty() && ver.chars().all(|c| c.is_ascii_digit())
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::super::email_verification::Entity")]
    EmailVerification,
    #[sea_orm(has_many = "super::super::forgot_password::Entity")]
    ForgotPassword,
    #[sea_orm(has_many = "super::super::post::Entity")]
    Post,
    #[sea_orm(
        belongs_to = "super::super::media::Entity",
        from = "Column::AvatarId",
        to = "super::super::media::Column::Id"
    )]
    Media,
}

impl Related<super::super::email_verification::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::EmailVerification.def()
    }
}

impl Related<super::super::forgot_password::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ForgotPassword.def()
    }
}

impl Related<super::super::post::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Post.def()
    }
}

impl Related<super::super::media::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Media.def()
    }
}

// before_save encrypts two_fa_secret (random nonce) and google_id (deterministic, for encrypt-then-lookup) on every write; the envelope check avoids double-wrapping existing ciphertext. Write path is untested.
#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(mut self, _db: &C, _insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        match self.two_fa_secret.clone() {
            sea_orm::ActiveValue::Set(Some(value)) => {
                let to_store = if looks_like_envelope(&value) {
                    value
                } else {
                    field_crypto::encrypt(&value).map_err(|err| {
                        DbErr::Custom(format!("users.two_fa_secret encryption failed: {err}"))
                    })?
                };
                self.two_fa_secret = sea_orm::ActiveValue::Set(Some(to_store));
            }
            sea_orm::ActiveValue::Set(None) | sea_orm::ActiveValue::Unchanged(_) => {}
            sea_orm::ActiveValue::NotSet => {}
        }

        match self.google_id.clone() {
            sea_orm::ActiveValue::Set(Some(value)) => {
                let to_store = if looks_like_envelope(&value) {
                    value
                } else {
                    field_crypto::encrypt_deterministic(&value).map_err(|err| {
                        DbErr::Custom(format!("users.google_id encryption failed: {err}"))
                    })?
                };
                self.google_id = sea_orm::ActiveValue::Set(Some(to_store));
            }
            sea_orm::ActiveValue::Set(None) | sea_orm::ActiveValue::Unchanged(_) => {}
            sea_orm::ActiveValue::NotSet => {}
        }

        let needs_backfill = match &self.session_auth_secret {
            sea_orm::ActiveValue::NotSet => true,
            sea_orm::ActiveValue::Set(s) if s.is_empty() => true,
            _ => false,
        };
        if needs_backfill {
            self.session_auth_secret =
                sea_orm::ActiveValue::Set(new_session_auth_secret().map_err(|err| {
                    DbErr::Custom(format!(
                        "users.session_auth_secret generation failed: {err}"
                    ))
                })?);
        }

        Ok(self)
    }
}

pub fn new_session_auth_secret() -> Result<String, String> {
    let mut buf = [0u8; 32];
    getrandom::getrandom(&mut buf).map_err(|e| format!("CSPRNG failure: {e}"))?;
    Ok(hex_encode(&buf))
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
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
    fn before_save_encrypts_two_fa_secret() {
        field_crypto::set_key(&test_key()).ok();
        let plaintext = "JBSWY3DPEHPK3PXP";
        let envelope = field_crypto::encrypt(plaintext).expect("encrypt");
        assert!(looks_like_envelope(&envelope), "envelope must be tagged");
        assert_ne!(envelope, plaintext);
        assert_eq!(
            decrypt_two_fa_secret_value(&envelope).expect("decrypt"),
            plaintext
        );
    }

    #[test]
    fn before_save_is_idempotent_on_envelope() {
        field_crypto::set_key(&test_key()).ok();
        let envelope = field_crypto::encrypt("seed").expect("encrypt");
        assert!(looks_like_envelope(&envelope));
        assert_eq!(
            decrypt_two_fa_secret_value(&envelope).expect("decrypt"),
            "seed"
        );
    }

    #[test]
    fn legacy_plaintext_two_fa_secret_is_returned_as_is() {
        let legacy = "JBSWY3DPEHPK3PXP";
        assert!(!looks_like_envelope(legacy));
        assert_eq!(decrypt_two_fa_secret_value(legacy).expect("legacy"), legacy);
    }

    #[test]
    fn google_id_deterministic_envelope_round_trips() {
        field_crypto::set_key(&test_key()).ok();
        let id = "1234567890";
        let envelope = field_crypto::encrypt_deterministic(id).expect("enc");
        assert!(looks_like_envelope(&envelope));
        assert_eq!(
            field_crypto::encrypt_deterministic(id).expect("enc"),
            envelope
        );
        assert_eq!(decrypt_google_id_value(&envelope).expect("dec"), id);
    }

    #[test]
    fn looks_like_envelope_detector() {
        assert!(looks_like_envelope("V1:abc"));
        assert!(looks_like_envelope("D1:abc"));
        assert!(looks_like_envelope("V42:abc"));
        assert!(!looks_like_envelope("JBSWY3DPEHPK3PXP"));
        assert!(!looks_like_envelope("1234567890"));
        assert!(!looks_like_envelope("Value"));
        assert!(!looks_like_envelope("Vx:abc"));
        assert!(!looks_like_envelope("plain"));
    }

    #[test]
    fn new_session_auth_secret_is_hex_and_unique() {
        let a = new_session_auth_secret().expect("CSPRNG available");
        let b = new_session_auth_secret().expect("CSPRNG available");
        assert_eq!(a.len(), 64, "must be 64 hex chars (256 bits)");
        assert!(
            a.chars().all(|c| c.is_ascii_hexdigit()),
            "must be lowercase hex"
        );
        assert_ne!(a, b, "must be CSPRNG-random, not constant");
        assert!(!a.is_empty());
    }
}
