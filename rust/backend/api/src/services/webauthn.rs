use std::sync::Arc;

use sea_orm::DatabaseConnection;
use tracing::warn;
use webauthn_rs::prelude::*;

use crate::db::sea_models::{passkey_credential, user};
use crate::error::{ErrorCode, ErrorResponse};

#[derive(Clone)]
pub struct WebauthnService {
    core: Arc<Webauthn>,
}

impl WebauthnService {
    pub fn from_env() -> Result<Self, ErrorResponse> {
        let rp_id = std::env::var("WEBAUTHN_RP_ID")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "localhost".to_string());
        let rp_origin = std::env::var("WEBAUTHN_RP_ORIGIN")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "http://localhost:8080".to_string());
        let rp_name = std::env::var("WEBAUTHN_RP_NAME")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "Ruxlog".to_string());
        Self::new(&rp_id, &rp_origin, &rp_name)
    }

    pub fn new(rp_id: &str, rp_origin: &str, rp_name: &str) -> Result<Self, ErrorResponse> {
        let origin = url::Url::parse(rp_origin).map_err(|e| {
            ErrorResponse::new(ErrorCode::ConfigurationError)
                .with_message(
                    "Invalid WEBAUTHN_RP_ORIGIN (expected a full URL like https://ruxlog.com)",
                )
                .with_details(e.to_string())
        })?;

        let builder = WebauthnBuilder::new(rp_id, &origin).map_err(map_webauthn_err)?;
        let builder = builder.rp_name(rp_name);
        let core = builder.build().map_err(map_webauthn_err)?;
        Ok(Self {
            core: Arc::new(core),
        })
    }

    pub fn start_registration(
        &self,
        user: &user::Model,
    ) -> Result<(CreationChallengeResponse, PasskeyRegistration), ErrorResponse> {
        let user_uuid = user_handle_for(user.id);
        let (challenge, state) = self
            .core
            .start_passkey_registration(user_uuid, &user.email, &user.name, None)
            .map_err(map_webauthn_err)?;
        Ok((challenge, state))
    }

    pub async fn finish_registration(
        &self,
        db: &DatabaseConnection,
        user_id: i32,
        reg: &RegisterPublicKeyCredential,
        state: &PasskeyRegistration,
        device_type: Option<String>,
        transports: Option<serde_json::Value>,
    ) -> Result<passkey_credential::Model, ErrorResponse> {
        let passkey = self
            .core
            .finish_passkey_registration(reg, state)
            .map_err(map_webauthn_err)?;
        passkey_credential::Entity::create(db, user_id, &passkey, device_type, transports).await
    }

    pub fn start_login(
        &self,
    ) -> Result<(RequestChallengeResponse, PasskeyAuthentication), ErrorResponse> {
        let (challenge, state) = self
            .core
            .start_passkey_authentication(&[])
            .map_err(map_webauthn_err)?;
        Ok((challenge, state))
    }

    pub async fn finish_login(
        &self,
        db: &DatabaseConnection,
        cred: &PublicKeyCredential,
        state: &PasskeyAuthentication,
    ) -> Result<(passkey_credential::Model, user::Model), ErrorResponse> {
        let result = self
            .core
            .finish_passkey_authentication(cred, state)
            .map_err(map_webauthn_err)?;

        if !result.user_verified() {
            warn!("passkey login rejected: user_verified is false");
            return Err(
                ErrorResponse::new(ErrorCode::Unauthorized).with_message("Authentication failed")
            );
        }

        let credential_id = passkey_credential::encode_credential_id(result.cred_id().as_slice());
        let stored = passkey_credential::Entity::find_by_credential_id(db, &credential_id)
            .await?
            .ok_or_else(|| {
                warn!(credential_id = %credential_id, "passkey login: unknown credential id");
                ErrorResponse::new(ErrorCode::Unauthorized).with_message("Authentication failed")
            })?;

        // Counter==0 authenticators have no counter support; the exemption is required or every login from them is rejected (WebAuthn §6.1.17).
        let counter = result.counter();
        if counter != 0 && (counter as i64) <= stored.counter {
            warn!(
                credential_id = %stored.credential_id,
                user_id = stored.user_id,
                stored_counter = stored.counter,
                asserted_counter = counter,
                "passkey login rejected: signature counter did not advance (possible cloned credential)"
            );
            return Err(
                ErrorResponse::new(ErrorCode::Unauthorized).with_message("Authentication failed")
            );
        }

        let user_model = user::Entity::get_by_id(db, stored.user_id)
            .await?
            .ok_or_else(|| {
                warn!(
                    user_id = stored.user_id,
                    "passkey login: credential points at missing user"
                );
                ErrorResponse::new(ErrorCode::Unauthorized).with_message("Authentication failed")
            })?;

        passkey_credential::Entity::touch_counter(db, stored.id, counter).await?;

        Ok((stored, user_model))
    }
}

// Keep the client error generic; the specific failure is logged only — surfacing it leaks signal to attackers.
fn map_webauthn_err(err: WebauthnError) -> ErrorResponse {
    warn!(error = ?err, "WebAuthn operation failed");
    ErrorResponse::new(ErrorCode::InvalidToken).with_message("WebAuthn operation failed")
}

fn user_handle_for(user_id: i32) -> Uuid {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(b"ruxlog:user:");
    hasher.update(user_id.to_be_bytes());
    let digest = hasher.finalize();

    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;

    Uuid::from_bytes(bytes)
}
