//! Shared device vectors (`contracts/device-vectors.json`, devices.md D3). The same
//! file drives `web/src/lib/device/__tests__/keys.test.ts`. A native client is ready
//! when it passes this file too.
use aws_lc_rs::hmac;
use base64::prelude::*;
use ruxlog::modules::device_v1::keys::{
    decode_exact, der_to_raw, parse_public_key, proof_message, verify_raw, PublicJwk,
    RAW_SIGNATURE_BYTES,
};
use serde::Deserialize;

const VECTORS: &str = include_str!("../../../../contracts/device-vectors.json");

#[derive(Deserialize)]
struct Vectors {
    signatures: Vec<SignatureCase>,
    der_to_raw: Vec<DerCase>,
    der_invalid: Vec<DerInvalidCase>,
    thumbprints: Vec<ThumbprintCase>,
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
struct DerInvalidCase {
    name: String,
    der_hex: String,
}

#[derive(Deserialize)]
struct ThumbprintCase {
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
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).expect("hex byte"))
        .collect()
}

/// Continuity hint normalisation: trim ASCII whitespace, then ASCII lower-case.
fn normalise_os_id(raw: &str) -> String {
    raw.trim_matches([' ', '\t', '\r', '\n'])
        .to_ascii_lowercase()
}

#[test]
fn signed_messages_match_the_proof_format_and_verify_only_when_valid() {
    let v = vectors();
    assert!(!v.signatures.is_empty());
    for case in &v.signatures {
        let message = proof_message(&case.nonce, &case.device_id, case.user_id);
        assert_eq!(
            message,
            case.message_utf8.as_bytes(),
            "{}: message",
            case.name
        );

        let raw = decode_exact(&case.raw_b64u, RAW_SIGNATURE_BYTES)
            .unwrap_or_else(|| panic!("{}: raw signature length", case.name));
        let key = parse_public_key(&case.public_jwk)
            .unwrap_or_else(|| panic!("{}: public key shape", case.name));
        assert_eq!(
            verify_raw(&key.point, &message, &raw),
            case.valid,
            "{}: verify",
            case.name
        );
    }
}

#[test]
fn every_der_signature_converts_to_the_raw_form_the_wire_carries() {
    let v = vectors();
    for case in &v.signatures {
        let der = BASE64_URL_SAFE_NO_PAD
            .decode(&case.der_b64u)
            .expect("der base64url");
        let raw = BASE64_URL_SAFE_NO_PAD
            .decode(&case.raw_b64u)
            .expect("raw base64url");
        let converted = der_to_raw(&der).unwrap_or_else(|| panic!("{}: DER parses", case.name));
        assert_eq!(converted.as_slice(), raw.as_slice(), "{}", case.name);
    }
}

#[test]
fn der_to_raw_pads_and_strips_integers_exactly() {
    let v = vectors();
    assert_eq!(v.der_to_raw.len(), 3, "padded, unpadded and stripped cases");
    for case in &v.der_to_raw {
        let converted =
            der_to_raw(&hex(&case.der_hex)).unwrap_or_else(|| panic!("{}: DER parses", case.name));
        let expected = BASE64_URL_SAFE_NO_PAD
            .decode(&case.raw_b64u)
            .expect("raw base64url");
        assert_eq!(converted.as_slice(), expected.as_slice(), "{}", case.name);
    }
}

#[test]
fn malformed_der_is_refused() {
    let v = vectors();
    for case in &v.der_invalid {
        assert!(
            der_to_raw(&hex(&case.der_hex)).is_none(),
            "{} must be refused",
            case.name
        );
    }
}

#[test]
fn thumbprints_follow_rfc_7638_over_the_four_members() {
    let v = vectors();
    for case in &v.thumbprints {
        let key = parse_public_key(&case.public_jwk)
            .unwrap_or_else(|| panic!("{}: public key shape", case.name));
        assert_eq!(key.jwk_text, case.canonical, "{}: canonical", case.name);
        assert_eq!(key.thumbprint, case.thumbprint, "{}: thumbprint", case.name);
    }
    assert_ne!(v.thumbprints[0].thumbprint, v.thumbprints[1].thumbprint);
}

#[test]
fn continuity_hints_are_hmac_over_the_normalised_os_id() {
    let v = vectors();
    for case in &v.hints {
        assert_eq!(
            normalise_os_id(&case.os_id),
            case.normalised,
            "{}: normalisation",
            case.name
        );
        let salt = BASE64_URL_SAFE_NO_PAD
            .decode(&case.salt_b64u)
            .expect("salt base64url");
        let key = hmac::Key::new(hmac::HMAC_SHA256, &salt);
        let tag = hmac::sign(&key, case.normalised.as_bytes());
        assert_eq!(
            BASE64_URL_SAFE_NO_PAD.encode(tag.as_ref()),
            case.hint,
            "{}: hint",
            case.name
        );
    }
}

#[test]
fn the_same_machine_hints_differently_under_two_accounts() {
    let v = vectors();
    let same_os: Vec<&HintCase> = v
        .hints
        .iter()
        .filter(|h| h.normalised == "0a1b2c-machine.guid")
        .collect();
    assert_eq!(same_os.len(), 3);
    assert_eq!(same_os[0].hint, same_os[1].hint, "same salt, same hint");
    assert_ne!(same_os[0].hint, same_os[2].hint, "other salt, other hint");
}
