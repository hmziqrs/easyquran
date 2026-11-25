//! Passkeys (PARITY I5): the codec between the server's WebAuthn JSON
//! (`passkey/v1/{register,login}/{begin,finish}`, webauthn-rs wire shapes)
//! and the plain requests and answers a platform authenticator works with.
//!
//! Challenges, user handles, credential ids and signatures stay out of
//! `Debug`, so a request or an answer can never reach a log through it.

use std::fmt;

use base64::{
    Engine,
    alphabet::URL_SAFE,
    engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig},
};
use serde_json::{Map, Value, json};

/// COSE algorithm ES256, the only one Apple's platform authenticator makes.
pub const ES256: i64 = -7;

const PUBLIC_KEY: &str = "public-key";
/// WebAuthn §13.4.3: a challenge is at least 16 bytes.
const MIN_CHALLENGE: usize = 16;
/// WebAuthn §4: a user handle is 1 to 64 bytes.
const MAX_USER_HANDLE: usize = 64;

/// base64url, written without padding, read with or without.
const B64URL: GeneralPurpose = GeneralPurpose::new(
    &URL_SAFE,
    GeneralPurposeConfig::new()
        .with_encode_padding(false)
        .with_decode_padding_mode(DecodePaddingMode::Indifferent),
);

/// What a message lacked or had wrong: the field's name, never its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("passkey: bad `{0}`")]
pub struct CodecError(pub &'static str);

/// Bytes the wire carries as base64url.
#[derive(Clone, PartialEq, Eq)]
pub struct Bytes(Vec<u8>);

impl Bytes {
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }

    fn encode(&self) -> String {
        B64URL.encode(&self.0)
    }

    /// Non-empty base64url text.
    fn decode(value: Option<&Value>, field: &'static str) -> Result<Self, CodecError> {
        value
            .and_then(Value::as_str)
            .and_then(|text| B64URL.decode(text).ok())
            .filter(|bytes| !bytes.is_empty())
            .map(Self)
            .ok_or(CodecError(field))
    }
}

impl From<Vec<u8>> for Bytes {
    fn from(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }
}

impl fmt::Debug for Bytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Bytes({})", self.0.len())
    }
}

/// A credential the server names (`allowCredentials`, `excludeCredentials`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Descriptor {
    pub id: Bytes,
    pub transports: Vec<String>,
}

/// `userVerification`; the server always asks for `Required`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserVerification {
    Required,
    Preferred,
    Discouraged,
}

impl UserVerification {
    /// Absent is WebAuthn's default; an unknown value is an error, so a
    /// stricter demand is never read as a weaker one.
    fn decode(value: Option<&Value>) -> Result<Self, CodecError> {
        match value {
            None | Some(Value::Null) => Ok(Self::Preferred),
            Some(Value::String(text)) => match text.as_str() {
                "required" => Ok(Self::Required),
                "preferred" => Ok(Self::Preferred),
                "discouraged" => Ok(Self::Discouraged),
                _ => Err(CodecError("userVerification")),
            },
            Some(_) => Err(CodecError("userVerification")),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Required => "required",
            Self::Preferred => "preferred",
            Self::Discouraged => "discouraged",
        }
    }
}

/// What a new passkey is made for (`register/begin`).
#[derive(Clone, PartialEq, Eq)]
pub struct CreationOptions {
    pub rp_id: String,
    pub rp_name: String,
    pub user_id: Bytes,
    pub user_name: String,
    pub user_display_name: String,
    pub challenge: Bytes,
    /// COSE algorithms, the server's most wanted first.
    pub algorithms: Vec<i64>,
    pub exclude: Vec<Descriptor>,
    pub user_verification: UserVerification,
    pub attestation: Option<String>,
    pub timeout_ms: Option<u32>,
}

impl fmt::Debug for CreationOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CreationOptions")
            .field("rp_id", &self.rp_id)
            .finish_non_exhaustive()
    }
}

/// What a sign-in asks the authenticator to sign (`login/begin`).
#[derive(Clone, PartialEq, Eq)]
pub struct RequestOptions {
    pub rp_id: String,
    pub challenge: Bytes,
    /// Empty: any passkey of the site (the server's discoverable sign-in).
    pub allow: Vec<Descriptor>,
    pub user_verification: UserVerification,
    pub timeout_ms: Option<u32>,
}

impl fmt::Debug for RequestOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RequestOptions")
            .field("rp_id", &self.rp_id)
            .finish_non_exhaustive()
    }
}

/// `challenge.publicKey` of a begin reply.
fn public_key(challenge: &Value) -> Result<&Map<String, Value>, CodecError> {
    challenge
        .get("publicKey")
        .and_then(Value::as_object)
        .ok_or(CodecError("publicKey"))
}

fn challenge_bytes(options: &Map<String, Value>) -> Result<Bytes, CodecError> {
    let bytes = Bytes::decode(options.get("challenge"), "challenge")?;
    if bytes.0.len() < MIN_CHALLENGE {
        return Err(CodecError("challenge"));
    }
    Ok(bytes)
}

fn text(value: Option<&Value>, field: &'static str) -> Result<String, CodecError> {
    value
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
        .ok_or(CodecError(field))
}

fn timeout(options: &Map<String, Value>) -> Option<u32> {
    options
        .get("timeout")
        .and_then(Value::as_u64)
        .and_then(|ms| u32::try_from(ms).ok())
}

/// A descriptor list. A malformed entry is an error: dropping one would
/// widen what the server allowed, or make a passkey it excluded.
fn descriptors(value: Option<&Value>, field: &'static str) -> Result<Vec<Descriptor>, CodecError> {
    let items = match value {
        None | Some(Value::Null) => return Ok(Vec::new()),
        Some(Value::Array(items)) => items,
        Some(_) => return Err(CodecError(field)),
    };
    items
        .iter()
        .map(|item| {
            if item.get("type").and_then(Value::as_str) != Some(PUBLIC_KEY) {
                return Err(CodecError(field));
            }
            Ok(Descriptor {
                id: Bytes::decode(item.get("id"), field)?,
                transports: transports_of(item.get("transports")),
            })
        })
        .collect()
}

/// Transport names; they are hints, so anything else is left out.
fn transports_of(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

fn descriptors_json(list: &[Descriptor]) -> Value {
    list.iter()
        .map(|d| {
            let mut item = json!({ "type": PUBLIC_KEY, "id": d.id.encode() });
            if !d.transports.is_empty() {
                item["transports"] = json!(d.transports);
            }
            item
        })
        .collect()
}

impl CreationOptions {
    /// The `challenge` of a `register/begin` reply.
    pub fn decode(challenge: &Value) -> Result<Self, CodecError> {
        let options = public_key(challenge)?;
        let rp = options.get("rp");
        let user = options.get("user");
        let user_id = Bytes::decode(user.and_then(|u| u.get("id")), "user.id")?;
        if user_id.0.len() > MAX_USER_HANDLE {
            return Err(CodecError("user.id"));
        }
        let rp_id = text(rp.and_then(|r| r.get("id")), "rp.id")?;
        let user_name = text(user.and_then(|u| u.get("name")), "user.name")?;
        let algorithms: Vec<i64> = options
            .get("pubKeyCredParams")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|p| p.get("type").and_then(Value::as_str) == Some(PUBLIC_KEY))
            .filter_map(|p| p.get("alg").and_then(Value::as_i64))
            .collect();
        if algorithms.is_empty() {
            return Err(CodecError("pubKeyCredParams"));
        }
        let selection = options.get("authenticatorSelection");
        Ok(Self {
            rp_name: text(rp.and_then(|r| r.get("name")), "rp.name")
                .unwrap_or_else(|_| rp_id.clone()),
            user_display_name: text(user.and_then(|u| u.get("displayName")), "user.displayName")
                .unwrap_or_else(|_| user_name.clone()),
            rp_id,
            user_id,
            user_name,
            challenge: challenge_bytes(options)?,
            algorithms,
            exclude: descriptors(options.get("excludeCredentials"), "excludeCredentials")?,
            user_verification: UserVerification::decode(
                selection.and_then(|s| s.get("userVerification")),
            )?,
            attestation: options
                .get("attestation")
                .and_then(Value::as_str)
                .map(str::to_owned),
            timeout_ms: timeout(options),
        })
    }

    /// `PublicKeyCredentialCreationOptionsJSON`, for authenticators that
    /// take the request as JSON. The server signs in without a username, so
    /// it wants a discoverable passkey.
    pub fn to_json(&self) -> String {
        let params: Value = self
            .algorithms
            .iter()
            .map(|alg| json!({ "type": PUBLIC_KEY, "alg": alg }))
            .collect();
        let mut options = json!({
            "rp": { "id": self.rp_id, "name": self.rp_name },
            "user": {
                "id": self.user_id.encode(),
                "name": self.user_name,
                "displayName": self.user_display_name,
            },
            "challenge": self.challenge.encode(),
            "pubKeyCredParams": params,
            "excludeCredentials": descriptors_json(&self.exclude),
            "authenticatorSelection": {
                "residentKey": "required",
                "requireResidentKey": true,
                "userVerification": self.user_verification.as_str(),
            },
        });
        if let Some(attestation) = &self.attestation {
            options["attestation"] = Value::from(attestation.as_str());
        }
        if let Some(ms) = self.timeout_ms {
            options["timeout"] = Value::from(ms);
        }
        options.to_string()
    }
}

impl RequestOptions {
    /// The `challenge` of a `login/begin` reply. Its `mediation` is a
    /// browser autofill hint and is not read (the web drops it too).
    pub fn decode(challenge: &Value) -> Result<Self, CodecError> {
        let options = public_key(challenge)?;
        Ok(Self {
            rp_id: text(options.get("rpId"), "rpId")?,
            challenge: challenge_bytes(options)?,
            allow: descriptors(options.get("allowCredentials"), "allowCredentials")?,
            user_verification: UserVerification::decode(options.get("userVerification"))?,
            timeout_ms: timeout(options),
        })
    }

    /// `PublicKeyCredentialRequestOptionsJSON`.
    pub fn to_json(&self) -> String {
        let mut options = json!({
            "rpId": self.rp_id,
            "challenge": self.challenge.encode(),
            "allowCredentials": descriptors_json(&self.allow),
            "userVerification": self.user_verification.as_str(),
        });
        if let Some(ms) = self.timeout_ms {
            options["timeout"] = Value::from(ms);
        }
        options.to_string()
    }
}

/// A passkey the authenticator made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attestation {
    pub credential_id: Bytes,
    pub client_data_json: Bytes,
    pub attestation_object: Bytes,
    /// What the platform reports, empty when it reports none.
    pub transports: Vec<String>,
}

/// A sign-in the authenticator signed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assertion {
    pub credential_id: Bytes,
    pub client_data_json: Bytes,
    pub authenticator_data: Bytes,
    pub signature: Bytes,
    pub user_handle: Option<Bytes>,
}

/// The credential id and `response` of a credential an authenticator
/// answered as JSON (`RegistrationResponseJSON`, `AuthenticationResponseJSON`).
fn credential_json(text: &str) -> Result<(Bytes, Value), CodecError> {
    let mut value: Value = serde_json::from_str(text).map_err(|_| CodecError("credential"))?;
    if value.get("type").and_then(Value::as_str) != Some(PUBLIC_KEY) {
        return Err(CodecError("type"));
    }
    let raw_id = Bytes::decode(value.get("rawId"), "rawId")?;
    if Bytes::decode(value.get("id"), "id")? != raw_id {
        return Err(CodecError("id"));
    }
    match value.get_mut("response").map(Value::take) {
        Some(response @ Value::Object(_)) => Ok((raw_id, response)),
        _ => Err(CodecError("response")),
    }
}

impl Attestation {
    /// A `RegistrationResponseJSON`.
    pub fn from_json(text: &str) -> Result<Self, CodecError> {
        let (credential_id, response) = credential_json(text)?;
        Ok(Self {
            credential_id,
            client_data_json: Bytes::decode(response.get("clientDataJSON"), "clientDataJSON")?,
            attestation_object: Bytes::decode(
                response.get("attestationObject"),
                "attestationObject",
            )?,
            transports: transports_of(response.get("transports")),
        })
    }

    /// The `credential` of `register/finish`.
    pub fn credential(&self) -> Value {
        let id = self.credential_id.encode();
        let mut credential = json!({
            "id": id,
            "rawId": id,
            "type": PUBLIC_KEY,
            "response": {
                "clientDataJSON": self.client_data_json.encode(),
                "attestationObject": self.attestation_object.encode(),
            },
        });
        if let Some(transports) = self.transports() {
            credential["response"]["transports"] = transports;
        }
        credential
    }

    /// The `transports` of `register/finish`, when the platform has any.
    pub fn transports(&self) -> Option<Value> {
        if self.transports.is_empty() {
            return None;
        }
        Some(json!(self.transports))
    }
}

impl Assertion {
    /// An `AuthenticationResponseJSON`.
    pub fn from_json(text: &str) -> Result<Self, CodecError> {
        let (credential_id, response) = credential_json(text)?;
        let user_handle = match response.get("userHandle") {
            None | Some(Value::Null) => None,
            handle => Some(Bytes::decode(handle, "userHandle")?),
        };
        Ok(Self {
            credential_id,
            client_data_json: Bytes::decode(response.get("clientDataJSON"), "clientDataJSON")?,
            authenticator_data: Bytes::decode(
                response.get("authenticatorData"),
                "authenticatorData",
            )?,
            signature: Bytes::decode(response.get("signature"), "signature")?,
            user_handle,
        })
    }

    /// The `credential` of `login/finish`.
    pub fn credential(&self) -> Value {
        let id = self.credential_id.encode();
        let mut credential = json!({
            "id": id,
            "rawId": id,
            "type": PUBLIC_KEY,
            "response": {
                "clientDataJSON": self.client_data_json.encode(),
                "authenticatorData": self.authenticator_data.encode(),
                "signature": self.signature.encode(),
            },
        });
        if let Some(handle) = &self.user_handle {
            credential["response"]["userHandle"] = Value::from(handle.encode());
        }
        credential
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 16 bytes that need both url-safe characters and padding.
    const CHALLENGE: &str = "-_-_-_-_-_-_-_-_-_-_-w";

    fn creation() -> Value {
        json!({ "publicKey": {
            "rp": { "name": "EasyQuran", "id": "easyquran.fyi" },
            "user": { "id": "AQIDBA", "name": "a@b.c", "displayName": "A" },
            "challenge": CHALLENGE,
            "pubKeyCredParams": [
                { "type": "public-key", "alg": -7 },
                { "type": "public-key", "alg": -257 },
            ],
            "timeout": 300_000,
            "authenticatorSelection": {
                "residentKey": "required",
                "requireResidentKey": true,
                "userVerification": "required",
            },
            "attestation": "none",
            "extensions": { "credProps": true },
        }})
    }

    fn request() -> Value {
        json!({
            "publicKey": {
                "challenge": CHALLENGE,
                "timeout": 300_000,
                "rpId": "easyquran.fyi",
                "allowCredentials": [],
                "userVerification": "required",
            },
            "mediation": "conditional",
        })
    }

    #[test]
    fn creation_options_read_and_write_the_servers_json() {
        let options = CreationOptions::decode(&creation()).expect("decodes");
        assert_eq!(options.rp_id, "easyquran.fyi");
        assert_eq!(options.user_id.as_slice(), [1, 2, 3, 4]);
        assert_eq!(options.challenge.as_slice().len(), 16);
        assert_eq!(options.algorithms, [ES256, -257]);
        assert_eq!(options.user_verification, UserVerification::Required);
        let written: Value = serde_json::from_str(&options.to_json()).expect("json");
        assert_eq!(written["challenge"], CHALLENGE);
        assert_eq!(written["user"]["id"], "AQIDBA");
        assert_eq!(written["authenticatorSelection"]["residentKey"], "required");
        let again = CreationOptions::decode(&json!({ "publicKey": written }));
        assert_eq!(again, Ok(options));
    }

    #[test]
    fn request_options_read_and_write_the_servers_json() {
        let options = RequestOptions::decode(&request()).expect("decodes");
        assert!(options.allow.is_empty());
        let written: Value = serde_json::from_str(&options.to_json()).expect("json");
        let again = RequestOptions::decode(&json!({ "publicKey": written }));
        assert_eq!(again, Ok(options));
    }

    #[test]
    fn options_that_would_weaken_the_request_are_refused() {
        let with = |pointer: &str, value: Value| {
            let mut options = request();
            *options.pointer_mut(pointer).expect("field") = value;
            RequestOptions::decode(&options).map(|_| ())
        };
        assert!(with("/publicKey/challenge", json!("AQID")).is_err());
        assert!(with("/publicKey/challenge", json!("not base64!")).is_err());
        assert!(with("/publicKey/rpId", json!("")).is_err());
        assert!(with("/publicKey/userVerification", json!("none")).is_err());
        assert!(with("/publicKey/allowCredentials", json!([{ "id": "AQ" }])).is_err());
        assert!(with("/publicKey/allowCredentials", json!("all")).is_err());
        let mut user = creation();
        user["publicKey"]["user"]["id"] = Value::from(B64URL.encode([7_u8; 65]));
        assert_eq!(CreationOptions::decode(&user), Err(CodecError("user.id")));
        assert!(CreationOptions::decode(&json!({})).is_err());
    }

    #[test]
    fn assertion_json_becomes_the_login_credential() {
        let answer = json!({
            "id": "-_8",
            "rawId": "-_8=",
            "type": "public-key",
            "authenticatorAttachment": "platform",
            "clientExtensionResults": {},
            "response": {
                "clientDataJSON": "e30",
                "authenticatorData": "AQI",
                "signature": "AwQ",
                "userHandle": "BQY",
            },
        });
        let assertion = Assertion::from_json(&answer.to_string()).expect("decodes");
        assert_eq!(
            assertion.credential(),
            json!({
                "id": "-_8",
                "rawId": "-_8",
                "type": "public-key",
                "response": {
                    "clientDataJSON": "e30",
                    "authenticatorData": "AQI",
                    "signature": "AwQ",
                    "userHandle": "BQY",
                },
            })
        );
        let with = |pointer: &str, value: Value| {
            let mut changed = answer.clone();
            *changed.pointer_mut(pointer).expect("field") = value;
            Assertion::from_json(&changed.to_string())
        };
        assert!(with("/id", json!("AQ")).is_err());
        assert!(with("/type", json!("password")).is_err());
        assert!(with("/response/signature", json!("")).is_err());
        let no_handle = with("/response/userHandle", Value::Null).expect("optional");
        assert!(
            no_handle.credential()["response"]
                .get("userHandle")
                .is_none()
        );
    }

    #[test]
    fn attestation_keeps_the_transports_the_platform_reports() {
        let attestation = Attestation {
            credential_id: vec![1].into(),
            client_data_json: b"{}".to_vec().into(),
            attestation_object: vec![2].into(),
            transports: Vec::new(),
        };
        assert_eq!(attestation.transports(), None);
        assert!(
            attestation.credential()["response"]
                .get("transports")
                .is_none()
        );
        let answer = json!({
            "id": "AQ", "rawId": "AQ", "type": "public-key",
            "response": {
                "clientDataJSON": "e30",
                "attestationObject": "Ag",
                "transports": ["internal", "hybrid"],
            },
        });
        let read = Attestation::from_json(&answer.to_string()).expect("decodes");
        assert_eq!(read.transports(), Some(json!(["internal", "hybrid"])));
        assert_eq!(read.credential_id, attestation.credential_id);
    }

    #[test]
    fn debug_shows_no_challenge_account_or_credential() {
        let options = CreationOptions::decode(&creation()).expect("decodes");
        let shown = format!("{options:?}");
        assert!(!shown.contains("a@b.c") && !shown.contains(CHALLENGE));
        assert_eq!(format!("{:?}", options.challenge), "Bytes(16)");
    }
}
