//! Device key proof (devices.md D3). One P-256 key per account per install. The
//! server stores the public half and its RFC 7638 thumbprint, and verifies a
//! signature over a session-bound nonce. Signatures are raw `r||s` (64 bytes), the
//! WebCrypto form. Native clients that get DER convert it first (`der_to_raw`).
//!
//! Nothing in this module logs or returns key or hint material.

use aws_lc_rs::signature::{UnparsedPublicKey, ECDSA_P256_SHA256_FIXED};
use base64::prelude::*;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Domain line of the signed message. Changing it is a new protocol version.
pub const PROOF_DOMAIN: &str = "easyquran-device-v1";
pub const RAW_SIGNATURE_BYTES: usize = 64;
pub const RANDOM_BYTES: usize = 32;
pub const POINT_BYTES: usize = 65;

/// The public half of a P-256 key, as the client sends it. Only these four members.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PublicJwk {
    pub kty: String,
    pub crv: String,
    pub x: String,
    pub y: String,
}

/// A JWK that passed the shape checks, with what recognition stores.
#[derive(Clone, Debug)]
pub struct ParsedKey {
    /// Canonical JWK text. Stored as `public_key`.
    pub jwk_text: String,
    /// RFC 7638 thumbprint, base64url. The match key.
    pub thumbprint: String,
    /// Uncompressed SEC1 point, `0x04 || x || y`, for verification.
    pub point: [u8; POINT_BYTES],
}

/// Decode unpadded base64url and insist on an exact byte length. Re-encoding must
/// give the same text, so there is one spelling per value.
pub fn decode_exact(text: &str, len: usize) -> Option<Vec<u8>> {
    let bytes = BASE64_URL_SAFE_NO_PAD.decode(text).ok()?;
    if bytes.len() != len || BASE64_URL_SAFE_NO_PAD.encode(&bytes) != text {
        return None;
    }
    Some(bytes)
}

fn canonical_jwk(x: &str, y: &str) -> String {
    // RFC 7638 member order, no whitespace. The coordinates are base64url, so they
    // need no escaping.
    format!(r#"{{"crv":"P-256","kty":"EC","x":"{x}","y":"{y}"}}"#)
}

pub fn thumbprint_of_canonical(canonical: &str) -> String {
    BASE64_URL_SAFE_NO_PAD.encode(Sha256::digest(canonical.as_bytes()))
}

/// Check the JWK is a P-256 public key with 32-byte coordinates and build its
/// stored forms. The point itself is checked when the signature verifies.
pub fn parse_public_key(jwk: &PublicJwk) -> Option<ParsedKey> {
    if jwk.kty != "EC" || jwk.crv != "P-256" {
        return None;
    }
    let x = decode_exact(&jwk.x, 32)?;
    let y = decode_exact(&jwk.y, 32)?;
    let canonical = canonical_jwk(&jwk.x, &jwk.y);
    let mut point = [0_u8; POINT_BYTES];
    point[0] = 0x04;
    point[1..33].copy_from_slice(&x);
    point[33..].copy_from_slice(&y);
    Some(ParsedKey {
        thumbprint: thumbprint_of_canonical(&canonical),
        jwk_text: canonical,
        point,
    })
}

/// The bytes that get signed: four UTF-8 lines, no trailing newline.
pub fn proof_message(nonce: &str, device_id: &str, user_id: i32) -> Vec<u8> {
    format!("{PROOF_DOMAIN}\n{nonce}\n{device_id}\n{user_id}").into_bytes()
}

/// Verify a raw `r||s` signature over `message` with the uncompressed point.
pub fn verify_raw(point: &[u8; POINT_BYTES], message: &[u8], raw_signature: &[u8]) -> bool {
    if raw_signature.len() != RAW_SIGNATURE_BYTES {
        return false;
    }
    UnparsedPublicKey::new(&ECDSA_P256_SHA256_FIXED, point.as_slice())
        .verify(message, raw_signature)
        .is_ok()
}

/// A fresh random value for nonces and salts, base64url.
pub fn random_b64() -> String {
    let mut bytes = [0_u8; RANDOM_BYTES];
    rand::rng().fill(&mut bytes);
    BASE64_URL_SAFE_NO_PAD.encode(bytes)
}

/// A continuity hint is 32 bytes, base64url, exactly as the client sends it.
pub fn valid_hint(text: &str) -> bool {
    decode_exact(text, 32).is_some()
}

/// DER ECDSA signature (`SEQUENCE { INTEGER r, INTEGER s }`) to raw `r||s`. iOS and
/// Android produce DER and must convert before sending. Only minimal encodings are
/// accepted, so one signature has one raw form.
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

/// Minimal DER length: short form, or one-byte long form for 128 to 255.
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
