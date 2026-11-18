//! Device key wire rules (contracts/devices.md D3), the native side of
//! `rust/backend/api/src/modules/device_v1/keys.rs` and
//! `web/src/lib/device/keys.ts`. Passed against `contracts/device-vectors.json`.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use p256::ecdsa::{
    Signature, SigningKey, VerifyingKey,
    signature::{Signer, Verifier},
};
use sha2::{Digest, Sha256};

use crate::api::{Secret, types::PublicJwk};

/// First line of the proof message.
pub const PROOF_DOMAIN: &str = "easyquran-device-v1";
/// Raw `r || s`.
pub const RAW_SIGNATURE_BYTES: usize = 64;

/// base64url without padding.
pub fn b64u_encode(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

/// base64url without padding; `None` when malformed.
pub fn b64u_decode(text: &str) -> Option<Vec<u8>> {
    URL_SAFE_NO_PAD.decode(text).ok()
}

/// base64url text that decodes to exactly `len` bytes.
pub fn decode_exact(text: &str, len: usize) -> Option<Vec<u8>> {
    b64u_decode(text).filter(|bytes| bytes.len() == len)
}

/// RFC 7638 canonical text of a P-256 key: `crv, kty, x, y`, no whitespace.
pub fn canonical_jwk(x: &str, y: &str) -> String {
    format!(r#"{{"crv":"P-256","kty":"EC","x":"{x}","y":"{y}"}}"#)
}

/// RFC 7638 thumbprint: SHA-256 of the canonical text, base64url.
pub fn thumbprint(jwk: &PublicJwk) -> String {
    b64u_encode(&Sha256::digest(canonical_jwk(&jwk.x, &jwk.y).as_bytes()))
}

/// The signed bytes: four UTF-8 lines joined by LF, no trailing newline.
pub fn proof_message(nonce: &str, device_id: &str, user_id: i32) -> Vec<u8> {
    format!("{PROOF_DOMAIN}\n{nonce}\n{device_id}\n{user_id}").into_bytes()
}

/// DER `SEQUENCE { INTEGER r, INTEGER s }` to raw `r || s`, minimal
/// encodings only (the backend's rule: one signature, one raw form). For
/// signatures from platform keystores (Secure Enclave, Android Keystore),
/// which produce DER.
pub fn der_to_raw(der: &[u8]) -> Option<[u8; RAW_SIGNATURE_BYTES]> {
    if *der.first()? != 0x30 {
        return None;
    }
    let (body_len, rest) = der_length(&der[1..])?;
    if rest.len() != body_len {
        return None;
    }
    let (r, rest) = der_integer(rest)?;
    let (s, rest) = der_integer(rest)?;
    if !rest.is_empty() {
        return None;
    }
    let mut out = [0_u8; RAW_SIGNATURE_BYTES];
    left_pad(&mut out[..32], r)?;
    left_pad(&mut out[32..], s)?;
    Some(out)
}

/// Short form, or one-byte long form for 128 to 255.
fn der_length(bytes: &[u8]) -> Option<(usize, &[u8])> {
    let first = *bytes.first()?;
    if first < 0x80 {
        return Some((usize::from(first), &bytes[1..]));
    }
    if first != 0x81 {
        return None;
    }
    let len = usize::from(*bytes.get(1)?);
    if len < 0x80 {
        return None;
    }
    Some((len, &bytes[2..]))
}

fn der_integer(bytes: &[u8]) -> Option<(&[u8], &[u8])> {
    if *bytes.first()? != 0x02 {
        return None;
    }
    let (len, rest) = der_length(&bytes[1..])?;
    if len == 0 || rest.len() < len {
        return None;
    }
    let (value, tail) = rest.split_at(len);
    if value[0] & 0x80 != 0 {
        return None;
    }
    let value = match value {
        [0, next, ..] if next & 0x80 != 0 => &value[1..],
        [0, ..] if value.len() > 1 => return None,
        _ => value,
    };
    Some((value, tail))
}

fn left_pad(dst: &mut [u8], value: &[u8]) -> Option<()> {
    if value.len() > dst.len() {
        return None;
    }
    let at = dst.len() - value.len();
    dst[at..].copy_from_slice(value);
    Some(())
}

/// Trim ASCII whitespace (space, tab, CR, LF), then ASCII lower-case.
pub fn normalise_os_id(raw: &str) -> String {
    raw.trim_matches([' ', '\t', '\r', '\n'])
        .to_ascii_lowercase()
}

/// `base64url(HMAC-SHA256(key = salt, message = normalised OS id))`.
pub fn continuity_hint(salt: &[u8], os_id: &str) -> String {
    // HMAC takes a key of any length.
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(salt).expect("hmac accepts any key");
    mac.update(normalise_os_id(os_id).as_bytes());
    b64u_encode(&mac.finalize().into_bytes())
}

/// Whether `raw` (64 bytes) verifies over `message` under `jwk`.
pub fn verify_raw(jwk: &PublicJwk, message: &[u8], raw: &[u8]) -> bool {
    if jwk.kty != "EC" || jwk.crv != "P-256" || raw.len() != RAW_SIGNATURE_BYTES {
        return false;
    }
    let (Some(x), Some(y)) = (decode_exact(&jwk.x, 32), decode_exact(&jwk.y, 32)) else {
        return false;
    };
    let mut point = [0_u8; 65];
    point[0] = 0x04;
    point[1..33].copy_from_slice(&x);
    point[33..].copy_from_slice(&y);
    let Ok(key) = VerifyingKey::from_sec1_bytes(&point) else {
        return false;
    };
    let Ok(signature) = Signature::from_slice(raw) else {
        return false;
    };
    key.verify(message, &signature).is_ok()
}

/// A P-256 device key pair held in software. `Debug` never shows the
/// private half; [`DeviceKey::to_secret`] is the only way out, for the
/// secret store.
#[derive(Clone)]
pub struct DeviceKey {
    signing: SigningKey,
}

impl std::fmt::Debug for DeviceKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceKey")
            .field("thumbprint", &thumbprint(&self.public_jwk()))
            .finish_non_exhaustive()
    }
}

impl DeviceKey {
    /// A fresh key from the OS random source.
    pub fn generate() -> Self {
        Self {
            signing: SigningKey::random(&mut rand_core::OsRng),
        }
    }

    /// The key from [`DeviceKey::to_secret`] text; `None` when malformed.
    pub fn from_secret(secret: &Secret) -> Option<Self> {
        let bytes = decode_exact(secret.expose(), 32)?;
        let signing = SigningKey::from_slice(&bytes).ok()?;
        Some(Self { signing })
    }

    /// The private scalar as base64url, for the secret store only.
    pub fn to_secret(&self) -> Secret {
        Secret::new(b64u_encode(&self.signing.to_bytes()))
    }

    /// The public JWK: exactly `kty`, `crv`, `x`, `y`.
    pub fn public_jwk(&self) -> PublicJwk {
        let point = self.signing.verifying_key().to_encoded_point(false);
        let bytes = point.as_bytes();
        PublicJwk {
            kty: "EC".into(),
            crv: "P-256".into(),
            x: b64u_encode(&bytes[1..33]),
            y: b64u_encode(&bytes[33..65]),
        }
    }

    /// ECDSA P-256 / SHA-256 over `message`, raw `r || s`, base64url.
    pub fn sign(&self, message: &[u8]) -> String {
        let signature: Signature = self.signing.sign(message);
        b64u_encode(&signature.to_bytes())
    }
}

#[cfg(test)]
mod tests {
    //! `contracts/device-vectors.json`, the same file the backend
    //! (`tests/device_vectors.rs`) and web (`keys.test.ts`) pass.

    use serde::Deserialize;

    use super::*;

    const VECTORS: &str = include_str!("../../../../../contracts/device-vectors.json");

    #[derive(Deserialize)]
    struct Vectors {
        proof_domain: String,
        signatures: Vec<SignatureCase>,
        der_to_raw: Vec<DerCase>,
        der_invalid: Vec<DerInvalid>,
        thumbprints: Vec<ThumbCase>,
        hints: Vec<HintCase>,
    }

    #[derive(Deserialize)]
    struct SignatureCase {
        name: String,
        public_jwk: PublicJwk,
        nonce: String,
        device_id: String,
        user_id: i32,
        message_utf8: String,
        raw_b64u: String,
        der_b64u: String,
        valid: bool,
    }

    #[derive(Deserialize)]
    struct DerCase {
        name: String,
        der_hex: String,
        raw_b64u: String,
    }

    #[derive(Deserialize)]
    struct DerInvalid {
        name: String,
        der_hex: String,
    }

    #[derive(Deserialize)]
    struct ThumbCase {
        name: String,
        public_jwk: PublicJwk,
        canonical: String,
        thumbprint: String,
    }

    #[derive(Deserialize)]
    struct HintCase {
        name: String,
        salt_b64u: String,
        os_id: String,
        normalised: String,
        hint: String,
    }

    fn vectors() -> Vectors {
        serde_json::from_str(VECTORS).expect("device vectors parse")
    }

    fn hex(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16).expect("hex"))
            .collect()
    }

    #[test]
    fn proof_messages_and_signatures_match_the_vectors() {
        let v = vectors();
        assert_eq!(v.proof_domain, PROOF_DOMAIN);
        assert!(!v.signatures.is_empty());
        for case in &v.signatures {
            let message = proof_message(&case.nonce, &case.device_id, case.user_id);
            assert_eq!(message, case.message_utf8.as_bytes(), "{}", case.name);
            let raw = decode_exact(&case.raw_b64u, RAW_SIGNATURE_BYTES).expect("raw length");
            assert_eq!(
                verify_raw(&case.public_jwk, &message, &raw),
                case.valid,
                "{}",
                case.name
            );
        }
    }

    #[test]
    fn der_signatures_convert_to_the_raw_wire_form() {
        let v = vectors();
        for case in &v.signatures {
            let der = b64u_decode(&case.der_b64u).expect("der");
            let raw = b64u_decode(&case.raw_b64u).expect("raw");
            assert_eq!(
                der_to_raw(&der).expect("parses").as_slice(),
                raw,
                "{}",
                case.name
            );
        }
        assert_eq!(v.der_to_raw.len(), 3);
        for case in &v.der_to_raw {
            let raw = b64u_decode(&case.raw_b64u).expect("raw");
            assert_eq!(
                der_to_raw(&hex(&case.der_hex)).expect("parses").as_slice(),
                raw,
                "{}",
                case.name
            );
        }
        for case in &v.der_invalid {
            assert!(
                der_to_raw(&hex(&case.der_hex)).is_none(),
                "{} must be refused",
                case.name
            );
        }
    }

    #[test]
    fn thumbprints_follow_rfc_7638() {
        let v = vectors();
        for case in &v.thumbprints {
            assert_eq!(
                canonical_jwk(&case.public_jwk.x, &case.public_jwk.y),
                case.canonical,
                "{}",
                case.name
            );
            assert_eq!(
                thumbprint(&case.public_jwk),
                case.thumbprint,
                "{}",
                case.name
            );
        }
    }

    #[test]
    fn continuity_hints_match_the_vectors() {
        let v = vectors();
        for case in &v.hints {
            assert_eq!(
                normalise_os_id(&case.os_id),
                case.normalised,
                "{}",
                case.name
            );
            let salt = b64u_decode(&case.salt_b64u).expect("salt");
            assert_eq!(
                continuity_hint(&salt, &case.os_id),
                case.hint,
                "{}",
                case.name
            );
            assert!(decode_exact(&case.hint, 32).is_some());
        }
    }

    #[test]
    fn a_generated_key_signs_what_the_server_verifies() {
        let key = DeviceKey::generate();
        let jwk = key.public_jwk();
        let message = proof_message("Z0K47RB1uzngqa1y5C4VTdK-_018-Rsfy-mqtNLTbhs", "dev-1", 7);
        let signature = key.sign(&message);
        assert_eq!(signature.len(), 86, "64 raw bytes as unpadded base64url");
        let raw = decode_exact(&signature, RAW_SIGNATURE_BYTES).unwrap();
        assert!(verify_raw(&jwk, &message, &raw));
        assert!(!verify_raw(&jwk, &proof_message("x", "dev-1", 7), &raw));
        let json = serde_json::to_value(&jwk).unwrap();
        let mut members: Vec<&String> = json.as_object().unwrap().keys().collect();
        members.sort();
        assert_eq!(members, ["crv", "kty", "x", "y"]);

        let back = DeviceKey::from_secret(&key.to_secret()).unwrap();
        assert_eq!(back.public_jwk(), jwk);
    }

    #[test]
    fn key_debug_shows_no_private_material() {
        let key = DeviceKey::generate();
        let private = key.to_secret();
        let shown = format!("{key:?}");
        assert!(!shown.contains(private.expose()));
        assert!(shown.contains("thumbprint"));
    }
}
