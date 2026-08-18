//! Finish payloads carry only an opaque server-issued state handle; the WebAuthn
//! state itself is stored server-side (services/passkey_state) and never
//! deserialized from the client.

use serde::{Deserialize, Serialize};
use validator::Validate;
use webauthn_rs::prelude::{PublicKeyCredential, RegisterPublicKeyCredential};

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct V1RegisterFinishPayload {
    pub credential: RegisterPublicKeyCredential,
    #[validate(length(min = 1, max = 128))]
    pub state_handle: String,
    #[validate(length(max = 128))]
    pub device_type: Option<String>,
    pub transports: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct V1LoginFinishPayload {
    pub credential: PublicKeyCredential,
    #[validate(length(min = 1, max = 128))]
    pub state_handle: String,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct V1RemovePasskeyPayload {
    #[validate(length(min = 1, max = 512))]
    pub credential_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const B64U: &str = "AA";
    const HANDLE: &str = "state-handle-0123456789abcdef";

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
    fn login_finish_accepts_camel_case_credential_and_handle() {
        let body = json!({
            "credential": login_credential_camel_case(),
            "state_handle": HANDLE
        });
        let payload: V1LoginFinishPayload =
            serde_json::from_value(body).expect("camelCase login finish must deserialize");
        assert_eq!(payload.credential.type_, "public-key");
        assert_eq!(payload.credential.raw_id.as_slice(), [0]);
        assert_eq!(payload.state_handle, HANDLE);
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
            "state_handle": HANDLE
        });
        assert!(
            serde_json::from_value::<V1LoginFinishPayload>(body).is_err(),
            "snake_case fields must be rejected — webauthn-rs uses camelCase"
        );
    }

    #[test]
    fn login_finish_requires_state_handle() {
        let body = json!({ "credential": login_credential_camel_case() });
        assert!(
            serde_json::from_value::<V1LoginFinishPayload>(body).is_err(),
            "missing state_handle must be rejected"
        );
    }

    #[test]
    fn login_finish_rejects_client_forged_authentication_state() {
        let body = json!({
            "credential": login_credential_camel_case(),
            "authentication_state": { "ast": { "credentials": [] } }
        });
        assert!(
            serde_json::from_value::<V1LoginFinishPayload>(body).is_err(),
            "a forged authentication_state carries no state_handle and must be rejected"
        );
    }

    #[test]
    fn login_finish_rejects_empty_state_handle() {
        let body = json!({
            "credential": login_credential_camel_case(),
            "state_handle": ""
        });
        let payload = serde_json::from_value::<V1LoginFinishPayload>(body).unwrap();
        assert!(
            payload.validate().is_err(),
            "empty state_handle must fail validation"
        );
    }

    #[test]
    fn register_finish_accepts_camel_case_credential_and_handle() {
        let body = json!({
            "credential": register_credential_camel_case(),
            "state_handle": HANDLE
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
            "state_handle": HANDLE
        });
        assert!(
            serde_json::from_value::<V1RegisterFinishPayload>(body).is_err(),
            "snake_case fields must be rejected — webauthn-rs uses camelCase"
        );
    }

    #[test]
    fn register_finish_requires_state_handle() {
        let body = json!({ "credential": register_credential_camel_case() });
        assert!(
            serde_json::from_value::<V1RegisterFinishPayload>(body).is_err(),
            "missing state_handle must be rejected"
        );
    }

    #[test]
    fn register_finish_rejects_client_forged_registration_state() {
        let body = json!({
            "credential": register_credential_camel_case(),
            "registration_state": { "rs": {} }
        });
        assert!(
            serde_json::from_value::<V1RegisterFinishPayload>(body).is_err(),
            "a forged registration_state carries no state_handle and must be rejected"
        );
    }

    #[test]
    fn register_finish_rejects_oversized_device_type() {
        let body = json!({
            "credential": register_credential_camel_case(),
            "state_handle": HANDLE,
            "device_type": "x".repeat(129)
        });
        let payload = serde_json::from_value::<V1RegisterFinishPayload>(body)
            .expect("parses before validate");
        assert!(
            payload.validate().is_err(),
            "device_type longer than 128 chars must fail validation"
        );
    }

    #[test]
    fn register_finish_accepts_max_device_type() {
        let body = json!({
            "credential": register_credential_camel_case(),
            "state_handle": HANDLE,
            "device_type": "x".repeat(128)
        });
        let payload = serde_json::from_value::<V1RegisterFinishPayload>(body)
            .expect("parses before validate");
        assert!(payload.validate().is_ok());
    }

    #[test]
    fn register_finish_rejects_oversized_state_handle() {
        let body = json!({
            "credential": register_credential_camel_case(),
            "state_handle": "x".repeat(129)
        });
        let payload = serde_json::from_value::<V1RegisterFinishPayload>(body)
            .expect("parses before validate");
        assert!(
            payload.validate().is_err(),
            "state_handle longer than 128 chars must fail validation"
        );
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
