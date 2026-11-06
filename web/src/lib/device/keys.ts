/**
 * Device key proof (devices.md D3). Mirrors `rust/backend/api/src/modules/device_v1/keys.rs`.
 * Both sides pass `contracts/device-vectors.json`, so a native client written from
 * the contract alone agrees with the server byte for byte.
 *
 * Nothing here logs or returns key material. The hint is for comparison only.
 */

export const PROOF_DOMAIN = "easyquran-device-v1";
export const RAW_SIGNATURE_BYTES = 64;

export interface PublicJwk {
  kty: "EC";
  crv: "P-256";
  x: string;
  y: string;
}

const encoder = new TextEncoder();

export function base64UrlEncode(bytes: Uint8Array): string {
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replace(/\+/gu, "-").replace(/\//gu, "_").replace(/=+$/u, "");
}

/** Strict: unpadded, no foreign characters, and one spelling per value. */
export function base64UrlDecode(text: string): Uint8Array<ArrayBuffer> | null {
  if (!/^[A-Za-z0-9_-]*$/u.test(text) || text.length % 4 === 1) return null;
  const padded = text.replace(/-/gu, "+").replace(/_/gu, "/");
  const pad = padded.length % 4 === 0 ? "" : "=".repeat(4 - (padded.length % 4));
  let binary: string;
  try {
    binary = atob(padded + pad);
  } catch {
    return null;
  }
  const bytes = Uint8Array.from(binary, (char) => char.charCodeAt(0));
  return base64UrlEncode(bytes) === text ? bytes : null;
}

/** RFC 7638 member order, no whitespace. The coordinates need no escaping. */
export function canonicalJwk(jwk: Pick<PublicJwk, "x" | "y">): string {
  return `{"crv":"P-256","kty":"EC","x":"${jwk.x}","y":"${jwk.y}"}`;
}

export async function thumbprintOf(jwk: Pick<PublicJwk, "x" | "y">): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", encoder.encode(canonicalJwk(jwk)));
  return base64UrlEncode(new Uint8Array(digest));
}

/** The bytes a device key signs: four UTF-8 lines, no trailing newline. */
export function proofMessage(
  nonce: string,
  deviceId: string,
  userId: number,
): Uint8Array<ArrayBuffer> {
  return encoder.encode(`${PROOF_DOMAIN}\n${nonce}\n${deviceId}\n${userId}`);
}

/**
 * DER ECDSA signature to raw `r||s`. WebCrypto already returns the raw form, so the
 * browser never calls this at runtime. Native clients that get DER must run it.
 * Only minimal INTEGER encodings are accepted, so one signature has one raw form.
 */
export function derToRaw(der: Uint8Array<ArrayBuffer>): Uint8Array<ArrayBuffer> | null {
  if (der[0] !== 0x30) return null;
  const outer = derLength(der.subarray(1));
  if (outer === null || outer.rest.length !== outer.length) return null;
  const r = derInteger(outer.rest);
  if (r === null) return null;
  const s = derInteger(r.rest);
  if (s === null || s.rest.length !== 0) return null;
  const out = new Uint8Array(RAW_SIGNATURE_BYTES);
  return leftPad(out.subarray(0, 32), r.value) && leftPad(out.subarray(32), s.value) ? out : null;
}

interface Read {
  value: Uint8Array;
  rest: Uint8Array;
}

interface Length {
  length: number;
  rest: Uint8Array;
}

/** Minimal DER length: short form, or one-byte long form for 128 to 255. */
function derLength(bytes: Uint8Array): Length | null {
  const first = bytes[0];
  if (first === undefined) return null;
  if (first < 0x80) return { length: first, rest: bytes.subarray(1) };
  if (first !== 0x81) return null;
  const len = bytes[1];
  if (len === undefined || len < 0x80) return null;
  return { length: len, rest: bytes.subarray(2) };
}

function derInteger(bytes: Uint8Array): Read | null {
  if (bytes[0] !== 0x02) return null;
  const len = derLength(bytes.subarray(1));
  if (len === null || len.length === 0 || len.rest.length < len.length) return null;
  const raw = len.rest.subarray(0, len.length);
  const rest = len.rest.subarray(len.length);
  const first = raw[0];
  if (first === undefined || (first & 0x80) !== 0) return null;
  const second = raw[1];
  if (first === 0 && raw.length > 1) {
    if (second === undefined || (second & 0x80) === 0) return null;
    return { value: raw.subarray(1), rest };
  }
  return { value: raw, rest };
}

function leftPad(dst: Uint8Array, value: Uint8Array): boolean {
  if (value.length > dst.length) return false;
  dst.fill(0);
  dst.set(value, dst.length - value.length);
  return true;
}

/** Continuity hint input: trim ASCII whitespace, then ASCII lower-case. */
export function normaliseOsId(raw: string): string {
  return raw.replace(/^[ \t\r\n]+|[ \t\r\n]+$/gu, "").replace(/[A-Z]/gu, (c) => c.toLowerCase());
}

/** HMAC-SHA256(per-account salt, normalised OS id), base64url. Native clients only. */
export async function continuityHint(salt: Uint8Array<ArrayBuffer>, osId: string): Promise<string> {
  const key = await crypto.subtle.importKey("raw", salt, { name: "HMAC", hash: "SHA-256" }, false, [
    "sign",
  ]);
  const tag = await crypto.subtle.sign("HMAC", key, encoder.encode(normaliseOsId(osId)));
  return base64UrlEncode(new Uint8Array(tag));
}

/** Verify a raw signature over the proof message. Used by tests and by the sign-in self-check. */
export async function verifyProof(
  jwk: PublicJwk,
  message: Uint8Array<ArrayBuffer>,
  rawSignature: Uint8Array<ArrayBuffer>,
): Promise<boolean> {
  const key = await crypto.subtle.importKey(
    "jwk",
    { ...jwk, ext: true },
    { name: "ECDSA", namedCurve: "P-256" },
    false,
    ["verify"],
  );
  return crypto.subtle.verify({ name: "ECDSA", hash: "SHA-256" }, key, rawSignature, message);
}
