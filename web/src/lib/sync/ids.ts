/**
 * Client-minted ids shared by every synced domain: random UUIDv4 entity ids
 * and deterministic UUIDv5 ids (default folders). The server validator only
 * accepts dashed-hex UUIDs, so every path here produces exactly that shape.
 */

/**
 * Manual UUID v4 (8-4-4-4-12 lowercase hex, version 4 + variant bits set) for
 * insecure origins where crypto.randomUUID is missing: the server validator
 * only accepts dashed-hex UUIDs, so a non-UUID fallback id would 422 every
 * sync round forever. Entropy source: crypto.getRandomValues when present,
 * else Math.random (best available there).
 */
function fallbackUuidV4(): string {
  const bytes = new Uint8Array(16);
  if ("getRandomValues" in crypto) crypto.getRandomValues(bytes);
  else for (let i = 0; i < bytes.length; i += 1) bytes[i] = Math.floor(Math.random() * 256);
  bytes[6] = ((bytes[6] ?? 0) & 0x0f) | 0x40; // version 4
  bytes[8] = ((bytes[8] ?? 0) & 0x3f) | 0x80; // variant 10xx
  const hex = Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0"));
  return `${hex.slice(0, 4).join("")}-${hex.slice(4, 6).join("")}-${hex.slice(6, 8).join("")}-${hex.slice(8, 10).join("")}-${hex.slice(10).join("")}`;
}

/** Client-minted entity id (bookmark, folder, thread, event). Distinct from the engine's queue-entry mutation id. */
export function newEntityId(): string {
  // `in` (not typeof): randomUUID is missing on insecure origins (LAN http dev).
  if ("randomUUID" in crypto) return crypto.randomUUID();
  return fallbackUuidV4();
}

/** Pure-JS SHA-1: UUIDv5 namespacing needs it synchronously, and crypto.subtle is async and missing on insecure origins. */
function sha1(input: Uint8Array): Uint8Array {
  const h = new Uint32Array([0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476, 0xc3d2e1f0]);
  const padded = new Uint8Array((((input.length + 8) >>> 6) + 1) << 6);
  padded.set(input);
  padded[input.length] = 0x80;
  const dv = new DataView(padded.buffer);
  dv.setUint32(padded.length - 8, Math.floor(input.length / 0x20000000));
  dv.setUint32(padded.length - 4, input.length << 3);
  const w = new Uint32Array(80);
  for (let off = 0; off < padded.length; off += 64) {
    for (let i = 0; i < 16; i += 1) w[i] = dv.getUint32(off + i * 4);
    for (let i = 16; i < 80; i += 1) {
      const v = (w[i - 3] ?? 0) ^ (w[i - 8] ?? 0) ^ (w[i - 14] ?? 0) ^ (w[i - 16] ?? 0);
      w[i] = (v << 1) | (v >>> 31);
    }
    let a = h[0] ?? 0;
    let b = h[1] ?? 0;
    let c = h[2] ?? 0;
    let d = h[3] ?? 0;
    let e = h[4] ?? 0;
    for (let i = 0; i < 80; i += 1) {
      let f = 0;
      let k = 0;
      if (i < 20) {
        f = (b & c) | (~b & d);
        k = 0x5a827999;
      } else if (i < 40) {
        f = b ^ c ^ d;
        k = 0x6ed9eba1;
      } else if (i < 60) {
        f = (b & c) | (b & d) | (c & d);
        k = 0x8f1bbcdc;
      } else {
        f = b ^ c ^ d;
        k = 0xca62c1d6;
      }
      const t = (((a << 5) | (a >>> 27)) + f + e + k + (w[i] ?? 0)) >>> 0;
      e = d;
      d = c;
      c = (b << 30) | (b >>> 2);
      b = a;
      a = t;
    }
    h[0] = ((h[0] ?? 0) + a) >>> 0;
    h[1] = ((h[1] ?? 0) + b) >>> 0;
    h[2] = ((h[2] ?? 0) + c) >>> 0;
    h[3] = ((h[3] ?? 0) + d) >>> 0;
    h[4] = ((h[4] ?? 0) + e) >>> 0;
  }
  const out = new Uint8Array(20);
  const odv = new DataView(out.buffer);
  for (let i = 0; i < 5; i += 1) odv.setUint32(i * 4, h[i] ?? 0);
  return out;
}

/**
 * RFC 4122 UUIDv5 (SHA-1 name-based). Deterministic ids every device derives
 * alike — mirrored by the server (`uuid::Uuid::new_v5`), so both sides agree.
 */
export function uuidV5(namespace: string, name: string): string {
  const hexNs = namespace.replaceAll("-", "");
  const ns = Uint8Array.from({ length: 16 }, (_, i) =>
    Number.parseInt(hexNs.slice(i * 2, i * 2 + 2), 16),
  );
  const nameBytes = new TextEncoder().encode(name);
  const input = new Uint8Array(ns.length + nameBytes.length);
  input.set(ns);
  input.set(nameBytes, ns.length);
  const bytes = sha1(input).slice(0, 16);
  bytes[6] = ((bytes[6] ?? 0) & 0x0f) | 0x50;
  bytes[8] = ((bytes[8] ?? 0) & 0x3f) | 0x80;
  const hex = Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}
