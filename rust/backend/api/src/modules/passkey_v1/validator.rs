//! Passkey state is deserialized from the client-held blob safely: webauthn-rs cryptographically binds each state to its challenge, so tampered/replayed state is rejected at /finish.

use serde::{Deserialize, Serialize};
use validator::Validate;
use webauthn_rs::prelude::{
    PasskeyAuthentication, PasskeyRegistration, PublicKeyCredential, RegisterPublicKeyCredential,
};

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct V1RegisterFinishPayload {
    pub credential: RegisterPublicKeyCredential,
    pub registration_state: PasskeyRegistration,
    #[validate(length(max = 128))]
    pub device_type: Option<String>,
    pub transports: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct V1LoginFinishPayload {
    pub credential: PublicKeyCredential,
    pub authentication_state: PasskeyAuthentication,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct V1RemovePasskeyPayload {
    #[validate(length(min = 1, max = 512))]
    pub credential_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sea_models::user;
    use crate::services::webauthn::WebauthnService;
    use chrono::TimeZone;
    use serde_json::json;

    const B64U: &str = "AA";

    fn make_user() -> user::Model {
        let now = chrono::Utc
            .with_ymd_and_hms(2026, 1, 1, 0, 0, 0)
            .unwrap()
            .fixed_offset();
        user::Model {
            id: 1,
            name: "Pk User".to_string(),
            email: "pk@example.com".to_string(),
            password: None,
            avatar_id: None,
            is_verified: true,
            role: user::UserRole::User,
            two_fa_enabled: false,
            two_fa_secret: None,
            two_fa_backup_codes: None,
            two_fa_last_totp_counter: None,
            google_id: None,
            oauth_provider: None,
            session_auth_secret: "test-secret".to_string(),
            created_at: now,
            updated_at: now,
        }
    }

    fn svc() -> WebauthnService {
        WebauthnService::new("example.com", "https://example.com", "Test")
            .expect("dev WebAuthn service must construct for a real origin")
    }

    fn real_authentication_state() -> serde_json::Value {
        let (_c, state) = svc().start_login().unwrap();
        serde_json::to_value(&state).unwrap()
    }

    fn real_registration_state() -> serde_json::Value {
        let (_c, state) = svc().start_registration(&make_user()).unwrap();
        serde_json::to_value(&state).unwrap()
    }

    fn login_credential_camel_case() -> serde_json::Value {
        json!({
            "id": "cred-1",
            "rawId": B64U,
            "type": "public-key",
            "response": {
                "clientDataJSON": B64U,
                "authenticatorData": B64U,
                "signature": B64U
            }
        })
    }

    fn register_credential_camel_case() -> serde_json::Value {
        json!({
            "id": "cred-2",
            "rawId": B64U,
            "type": "public-key",
            "response": {
                "clientDataJSON": B64U,
                "attestationObject": B64U
            }
        })
    }

    #[test]
    fn login_finish_accepts_camel_case_credential() {
        let body = json!({
            "credential": login_credential_camel_case(),
            "authentication_state": real_authentication_state()
        });
        let payload: V1LoginFinishPayload =
            serde_json::from_value(body).expect("camelCase login finish must deserialize");
        assert_eq!(payload.credential.type_, "public-key");
        assert_eq!(payload.credential.raw_id.as_slice(), [0]);
    }

    #[test]
    fn login_finish_rejects_snake_case_credential_fields() {
        let body = json!({
            "credential": {
                "id": "cred-1",
                "raw_id": B64U,
                "type": "public-key",
                "response": {
                    "client_data_json": B64U,
                    "authenticator_data": B64U,
                    "signature": B64U
                }
            },
            "authentication_state": real_authentication_state()
        });
        assert!(
            serde_json::from_value::<V1LoginFinishPayload>(body).is_err(),
            "snake_case fields must be rejected — webauthn-rs uses camelCase"
        );
    }

    #[test]
    fn login_finish_requires_authentication_state() {
        let body = json!({ "credential": login_credential_camel_case() });
        assert!(
            serde_json::from_value::<V1LoginFinishPayload>(body).is_err(),
            "missing authentication_state must be rejected"
        );
    }

    #[test]
    fn register_finish_accepts_camel_case_credential() {
        let body = json!({
            "credential": register_credential_camel_case(),
            "registration_state": real_registration_state()
        });
        let payload: V1RegisterFinishPayload =
            serde_json::from_value(body).expect("camelCase register finish must deserialize");
        assert_eq!(payload.credential.type_, "public-key");
        assert!(payload.device_type.is_none());
    }

    #[test]
    fn register_finish_rejects_snake_case_credential_fields() {
        let body = json!({
            "credential": {
                "id": "cred-2",
                "raw_id": B64U,
                "type": "public-key",
                "response": {
                    "client_data_json": B64U,
                    "attestation_object": B64U
                }
            },
            "registration_state": real_registration_state()
        });
        assert!(
            serde_json::from_value::<V1RegisterFinishPayload>(body).is_err(),
            "snake_case fields must be rejected — webauthn-rs uses camelCase"
        );
    }

    #[test]
    fn register_finish_requires_registration_state() {
        let body = json!({ "credential": register_credential_camel_case() });
        assert!(
            serde_json::from_value::<V1RegisterFinishPayload>(body).is_err(),
            "missing registration_state must be rejected"
        );
    }

    #[test]
    fn register_finish_rejects_oversized_device_type() {
        let body = json!({
            "credential": register_credential_camel_case(),
            "registration_state": real_registration_state(),
            "device_type": "x".repeat(129)
        });
        let payload =
            serde_json::from_value::<V1RegisterFinishPayload>(body).expect("parses before validate");
        assert!(
            payload.validate().is_err(),
            "device_type longer than 128 chars must fail validation"
        );
    }

    #[test]
    fn register_finish_accepts_max_device_type() {
        let body = json!({
            "credential": register_credential_camel_case(),
            "registration_state": real_registration_state(),
            "device_type": "x".repeat(128)
        });
        let payload =
            serde_json::from_value::<V1RegisterFinishPayload>(body).expect("parses before validate");
        assert!(payload.validate().is_ok());
    }

    #[test]
    fn remove_payload_accepts_valid_credential_id() {
        let payload = V1RemovePasskeyPayload {
            credential_id: "cred-abc".to_string(),
        };
        assert!(payload.validate().is_ok());
    }

    #[test]
    fn remove_payload_rejects_empty_credential_id() {
        let payload = V1RemovePasskeyPayload {
            credential_id: String::new(),
        };
        assert!(
            payload.validate().is_err(),
            "empty credential_id must fail validation"
        );
    }

    #[test]
    fn remove_payload_rejects_oversized_credential_id() {
        let payload = V1RemovePasskeyPayload {
            credential_id: "x".repeat(513),
        };
        assert!(
            payload.validate().is_err(),
            "credential_id longer than 512 chars must fail validation"
        );
    }

    #[test]
    fn remove_payload_accepts_max_length_credential_id() {
        let payload = V1RemovePasskeyPayload {
            credential_id: "x".repeat(512),
        };
        assert!(payload.validate().is_ok());
    }

    #[test]
    fn remove_payload_uses_credential_id_key_not_id() {
        // The web client must send { "credential_id": ... } — an "id" alias is NOT defined.
        let body = json!({ "id": "cred-abc" });
        assert!(
            serde_json::from_value::<V1RemovePasskeyPayload>(body).is_err(),
            "remove payload must require credential_id, not id"
        );
    }
}
