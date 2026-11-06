/**
 * Client-minted ids shared by every synced domain: time-ordered UUID v7 entity
 * ids and deterministic UUID v5 ids (default folders). The server validators
 * only accept dashed-hex UUIDs, so every path here produces exactly that shape.
 *
 * An id's timestamp comes from the device clock: it sets sort order only. Never
 * trust it for security or read it as `startedAt`.
 */

const COUNTER_MAX = 0xfff;
/** A new millisecond starts below this, leaving room for the counter to climb. */
const COUNTER_START_LIMIT = 2048;
const TAIL_BYTES = 8;
const UUID_V7_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/iu;

/** `getRandomValues` where present, else Math.random (insecure origins, best available there). */
function fillRandom(bytes: Uint8Array): void {
  if ("getRandomValues" in crypto) {
    crypto.getRandomValues(bytes);
    return;
  }
  for (let i = 0; i < bytes.length; i += 1) bytes[i] = Math.floor(Math.random() * 256);
}

function randomBytes(length: number): Uint8Array {
  const bytes = new Uint8Array(length);
  fillRandom(bytes);
  return bytes;
}

function hexOf(bytes: Uint8Array): string {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

/**
 * RFC 9562 layout: 48-bit Unix ms, version 7, 12-bit `rand_a` counter, variant
 * `10`, then 62 bits from `tail`. Pure, and pinned by `contracts/uuid-v7.json`.
 */
export function encodeUuidV7(ms: number, counter: number, tail: Uint8Array): string {
  const time = Math.trunc(ms).toString(16).padStart(12, "0");
  const randA = counter.toString(16).padStart(3, "0");
  const variant = (((tail[0] ?? 0) & 0x3f) | 0x80).toString(16).padStart(2, "0");
  const rest = hexOf(tail.slice(1, TAIL_BYTES));
  return `${time.slice(0, 8)}-${time.slice(8)}-7${randA}-${variant}${rest.slice(0, 2)}-${rest.slice(2)}`;
}

export function isUuidV7(id: string): boolean {
  return UUID_V7_RE.test(id);
}

export interface UuidV7Source {
  readonly now: () => number;
  readonly random: (length: number) => Uint8Array;
}

function counterStart(bytes: Uint8Array): number {
  return (((bytes[0] ?? 0) << 8) | (bytes[1] ?? 0)) % COUNTER_START_LIMIT;
}

/**
 * Monotonic minter: ids from one generator never go down, even when the clock
 * moves backwards (the last ms is kept) or a ms runs out of counter (overflow
 * moves to the next ms).
 */
export function createUuidV7Generator(
  source: UuidV7Source = { now: () => Date.now(), random: randomBytes },
): () => string {
  let lastMs = -1;
  let counter = 0;
  return () => {
    const now = source.now();
    if (now > lastMs) {
      lastMs = now;
      counter = counterStart(source.random(2));
    } else if (counter >= COUNTER_MAX) {
      lastMs += 1;
      counter = counterStart(source.random(2));
    } else {
      counter += 1;
    }
    return encodeUuidV7(lastMs, counter, source.random(TAIL_BYTES));
  };
}

/** Client-minted entity id (bookmark, folder, thread, event). Distinct from the engine's queue-entry mutation id. */
export const newEntityId: () => string = createUuidV7Generator();

/**
 * Stable v7 for a stored non-v7 id: time from `ms`, the rest from the old id's
 * own bytes. Every tab and device re-keying one row lands on one id.
 */
export function uuidV7FromLegacy(legacyId: string, ms: number): string {
  const hex = legacyId.replaceAll("-", "").padEnd(32, "0");
  const tail = Uint8Array.from({ length: TAIL_BYTES }, (_, i) =>
    Number.parseInt(hex.slice(16 + i * 2, 18 + i * 2), 16),
  );
  const counter = Number.parseInt(hex.slice(13, 16), 16);
  return encodeUuidV7(Math.max(0, ms), counter, tail);
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
