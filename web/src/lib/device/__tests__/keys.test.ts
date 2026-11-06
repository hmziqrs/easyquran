import { describe, expect, it } from "vite-plus/test";

import rawVectors from "../../../../../contracts/device-vectors.json";
import {
  base64UrlDecode,
  base64UrlEncode,
  canonicalJwk,
  continuityHint,
  derToRaw,
  normaliseOsId,
  type PublicJwk,
  proofMessage,
  thumbprintOf,
  verifyProof,
} from "../keys";

interface SignatureVector {
  name: string;
  public_jwk: PublicJwk;
  nonce: string;
  device_id: string;
  user_id: number;
  message_utf8: string;
  raw_b64u: string;
  der_b64u: string;
  valid: boolean;
}

interface Vectors {
  signatures: SignatureVector[];
  der_to_raw: { name: string; der_hex: string; raw_b64u: string }[];
  der_invalid: { name: string; der_hex: string }[];
  thumbprints: { name: string; public_jwk: PublicJwk; canonical: string; thumbprint: string }[];
  hints: { name: string; salt_b64u: string; os_id: string; normalised: string; hint: string }[];
}

// SAFETY: the vector file is pinned in contracts/ and its shape is enforced by the Rust suite.
const vectors = rawVectors as Vectors;

function hex(text: string): Uint8Array<ArrayBuffer> {
  return Uint8Array.from(text.match(/../gu) ?? [], (pair) => Number.parseInt(pair, 16));
}

function b64u(text: string): Uint8Array<ArrayBuffer> {
  const bytes = base64UrlDecode(text);
  if (bytes === null) throw new Error(`vector is not base64url: ${text}`);
  return bytes;
}

describe("base64url", () => {
  it("is strict: one spelling per value, no padding, no foreign characters", () => {
    expect(base64UrlDecode(base64UrlEncode(new Uint8Array([251, 255, 0])))).toEqual(
      new Uint8Array([251, 255, 0]),
    );
    expect(base64UrlDecode("AAA=")).toBeNull();
    expect(base64UrlDecode("a+b/")).toBeNull();
    expect(base64UrlDecode("A")).toBeNull();
    // Trailing bits must be zero, or two strings would name one value.
    expect(base64UrlDecode("AB")).toBeNull();
  });
});

describe("shared device vectors", () => {
  it("builds the proof message exactly as the contract says", () => {
    for (const sig of vectors.signatures) {
      const message = proofMessage(sig.nonce, sig.device_id, sig.user_id);
      expect(new TextDecoder().decode(message)).toBe(sig.message_utf8);
    }
  });

  it("verifies each raw signature under its key, and only the valid ones", async () => {
    for (const sig of vectors.signatures) {
      const message = proofMessage(sig.nonce, sig.device_id, sig.user_id);
      const ok = await verifyProof(sig.public_jwk, message, b64u(sig.raw_b64u));
      expect(ok, sig.name).toBe(sig.valid);
    }
  });

  it("converts every DER signature to the raw form the server verifies", () => {
    for (const sig of vectors.signatures) {
      expect(derToRaw(b64u(sig.der_b64u)), sig.name).toEqual(b64u(sig.raw_b64u));
    }
  });

  it("pads and strips DER integers exactly", () => {
    expect(vectors.der_to_raw).toHaveLength(3);
    for (const entry of vectors.der_to_raw) {
      expect(derToRaw(hex(entry.der_hex)), entry.name).toEqual(b64u(entry.raw_b64u));
    }
  });

  it("refuses malformed DER", () => {
    for (const entry of vectors.der_invalid) {
      expect(derToRaw(hex(entry.der_hex)), entry.name).toBeNull();
    }
  });

  it("computes RFC 7638 thumbprints over the four members", async () => {
    for (const entry of vectors.thumbprints) {
      expect(canonicalJwk(entry.public_jwk), entry.name).toBe(entry.canonical);
      expect(await thumbprintOf(entry.public_jwk), entry.name).toBe(entry.thumbprint);
    }
  });

  it("derives continuity hints from the normalised OS id and the account salt", async () => {
    for (const entry of vectors.hints) {
      expect(normaliseOsId(entry.os_id), entry.name).toBe(entry.normalised);
      expect(await continuityHint(b64u(entry.salt_b64u), entry.os_id), entry.name).toBe(entry.hint);
    }
  });

  it("gives the same machine different hints under two accounts", () => {
    const same = vectors.hints.filter((h) => h.normalised === "0a1b2c-machine.guid");
    expect(same).toHaveLength(3);
    expect(same[0]?.hint).toBe(same[1]?.hint);
    expect(same[0]?.hint).not.toBe(same[2]?.hint);
  });
});
