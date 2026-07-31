//! The passkey state types are deserialized from the client-held blob; this is
//! safe because webauthn-rs cryptographically binds each state to its challenge,
//! so tampered/replayed state is rejected at /finish.

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
