//! Client-minted ids shared by every synced domain (web `lib/sync/ids.ts`):
//! time-ordered UUID v7 entity ids, pinned by `contracts/uuid-v7.json`, and
//! deterministic UUID v5 ids (default bookmark folders) the server derives
//! the same way. An id's timestamp comes from the device clock and only sets
//! sort order: never read it as `startedAt` or trust it for security.

use std::{
    collections::hash_map::RandomState,
    hash::{BuildHasher, Hasher},
};

const COUNTER_MAX: u16 = 0xfff;
/// A new millisecond starts below this, leaving room for the counter to climb.
const COUNTER_START_LIMIT: u16 = 2048;
const TAIL_BYTES: usize = 8;

/// Randomness for ids. Injected so tests replay exact ids.
pub trait Randomness {
    fn fill(&mut self, bytes: &mut [u8]);
}

/// OS-seeded randomness from std's `RandomState` keys. Ids need uniqueness,
/// not secrecy: never use this for keys or tokens.
#[derive(Debug, Default)]
pub struct OsRandom {
    counter: u64,
}

impl Randomness for OsRandom {
    fn fill(&mut self, bytes: &mut [u8]) {
        for chunk in bytes.chunks_mut(8) {
            self.counter = self.counter.wrapping_add(1);
            let mut hasher = RandomState::new().build_hasher();
            hasher.write_u64(self.counter);
            let word = hasher.finish().to_le_bytes();
            chunk.copy_from_slice(&word[..chunk.len()]);
        }
    }
}

/// Deterministic randomness for tests: a xorshift stream from a seed.
#[derive(Debug, Clone)]
pub struct SeededRandom(u64);

impl SeededRandom {
    pub fn new(seed: u64) -> Self {
        Self(seed | 1)
    }
}

impl Randomness for SeededRandom {
    fn fill(&mut self, bytes: &mut [u8]) {
        for byte in bytes {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            *byte = (self.0 >> 24) as u8;
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// RFC 9562 layout: 48-bit Unix ms, version 7, 12-bit `rand_a` counter,
/// variant `10`, then 62 bits from `tail`. Pure; pinned by `contracts/uuid-v7.json`.
pub fn encode_uuid_v7(ms: u64, counter: u16, tail: [u8; 8]) -> String {
    let time = format!("{:012x}", ms & 0xffff_ffff_ffff);
    let rand_a = format!("{:03x}", counter & COUNTER_MAX);
    let variant = (tail[0] & 0x3f) | 0x80;
    let rest = hex(&tail[1..]);
    format!(
        "{}-{}-7{}-{:02x}{}-{}",
        &time[..8],
        &time[8..],
        rand_a,
        variant,
        &rest[..2],
        &rest[2..]
    )
}

/// Dashed lowercase or uppercase hex, version 7, variant `10xx`.
pub fn is_uuid_v7(id: &str) -> bool {
    let bytes = id.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    let shape = bytes.iter().enumerate().all(|(i, b)| match i {
        8 | 13 | 18 | 23 => *b == b'-',
        _ => b.is_ascii_hexdigit(),
    });
    shape && bytes[14] == b'7' && matches!(bytes[19], b'8' | b'9' | b'a' | b'b' | b'A' | b'B')
}

fn counter_start(bytes: [u8; 2]) -> u16 {
    ((u16::from(bytes[0]) << 8) | u16::from(bytes[1])) % COUNTER_START_LIMIT
}

/// Monotonic minter: ids from one generator never go down, even when the
/// clock moves backwards (the last ms is kept) or a ms runs out of counter
/// (overflow moves to the next ms).
#[derive(Debug, Clone)]
pub struct UuidV7Generator {
    last_ms: i64,
    counter: u16,
}

impl Default for UuidV7Generator {
    fn default() -> Self {
        Self {
            last_ms: -1,
            counter: 0,
        }
    }
}

impl UuidV7Generator {
    pub fn new() -> Self {
        Self::default()
    }

    /// The next id at `now_ms` (device clock).
    pub fn next(&mut self, now_ms: i64, random: &mut dyn Randomness) -> String {
        let mut start = [0u8; 2];
        if now_ms > self.last_ms {
            self.last_ms = now_ms;
            random.fill(&mut start);
            self.counter = counter_start(start);
        } else if self.counter >= COUNTER_MAX {
            self.last_ms += 1;
            random.fill(&mut start);
            self.counter = counter_start(start);
        } else {
            self.counter += 1;
        }
        let mut tail = [0u8; TAIL_BYTES];
        random.fill(&mut tail);
        encode_uuid_v7(self.last_ms.max(0) as u64, self.counter, tail)
    }
}

/// Stable v7 for a stored non-v7 id: time from `ms`, the rest from the old
/// id's own bytes, so every device re-keying one row lands on one id.
pub fn uuid_v7_from_legacy(legacy_id: &str, ms: i64) -> String {
    let mut hex_digits: String = legacy_id.chars().filter(|c| *c != '-').collect();
    while hex_digits.len() < 32 {
        hex_digits.push('0');
    }
    let byte =
        |i: usize| u8::from_str_radix(hex_digits.get(i..i + 2).unwrap_or("00"), 16).unwrap_or(0);
    let mut tail = [0u8; TAIL_BYTES];
    for (i, slot) in tail.iter_mut().enumerate() {
        *slot = byte(16 + i * 2);
    }
    let counter = u16::from_str_radix(hex_digits.get(13..16).unwrap_or("000"), 16).unwrap_or(0);
    encode_uuid_v7(ms.max(0) as u64, counter, tail)
}

/// A random RFC 4122 v4 id (home card ids, as the web's `crypto.randomUUID`).
pub fn uuid_v4(random: &mut dyn Randomness) -> String {
    let mut bytes = [0u8; 16];
    random.fill(&mut bytes);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    dashed(&bytes)
}

fn dashed(bytes: &[u8; 16]) -> String {
    let h = hex(bytes);
    format!(
        "{}-{}-{}-{}-{}",
        &h[..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..]
    )
}

/// SHA-1, for UUID v5 name-based ids only (never over Quran data).
fn sha1(input: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [
        0x6745_2301,
        0xefcd_ab89,
        0x98ba_dcfe,
        0x1032_5476,
        0xc3d2_e1f0,
    ];
    let mut padded = input.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&((input.len() as u64) * 8).to_be_bytes());
    for block in padded.chunks(64) {
        let mut w = [0u32; 80];
        for (i, word) in block.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5a82_7999),
                20..=39 => (b ^ c ^ d, 0x6ed9_eba1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8f1b_bcdc),
                _ => (b ^ c ^ d, 0xca62_c1d6),
            };
            let t = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }
        for (slot, add) in h.iter_mut().zip([a, b, c, d, e]) {
            *slot = slot.wrapping_add(add);
        }
    }
    let mut out = [0u8; 20];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

/// RFC 4122 UUID v5 (SHA-1 name-based): ids every device derives alike,
/// mirrored by the server (`uuid::Uuid::new_v5`).
pub fn uuid_v5(namespace: &str, name: &str) -> String {
    let ns: String = namespace.chars().filter(|c| *c != '-').collect();
    let mut input: Vec<u8> = (0..16)
        .map(|i| u8::from_str_radix(ns.get(i * 2..i * 2 + 2).unwrap_or("00"), 16).unwrap_or(0))
        .collect();
    input.extend_from_slice(name.as_bytes());
    let digest = sha1(&input);
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    dashed(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const VECTORS: &str = include_str!("../../../../../contracts/uuid-v7.json");

    fn tail_of(hex_str: &str) -> [u8; 8] {
        let mut tail = [0u8; 8];
        for (i, slot) in tail.iter_mut().enumerate() {
            *slot = u8::from_str_radix(&hex_str[i * 2..i * 2 + 2], 16).unwrap();
        }
        tail
    }

    #[test]
    fn shared_uuid_v7_vectors_pass() {
        let doc: Value = serde_json::from_str(VECTORS).unwrap();
        let vectors = doc["vectors"].as_array().unwrap();
        assert!(!vectors.is_empty());
        for v in vectors {
            let id = encode_uuid_v7(
                v["ms"].as_u64().unwrap(),
                v["counter"].as_u64().unwrap() as u16,
                tail_of(v["random"].as_str().unwrap()),
            );
            assert_eq!(id, v["expected"].as_str().unwrap(), "{}", v["note"]);
            assert!(is_uuid_v7(&id));
        }
    }

    #[test]
    fn generator_is_monotonic_across_backwards_clock_and_overflow() {
        let mut generator = UuidV7Generator::new();
        let mut random = SeededRandom::new(7);
        let mut last = String::new();
        let times = [1_000, 1_000, 1_000, 999, 500, 2_000, 2_000];
        for now in times {
            let id = generator.next(now, &mut random);
            assert!(is_uuid_v7(&id));
            assert!(id > last, "{id} after {last}");
            last = id;
        }
        // Run a ms out of counter: the next id moves to the next ms and still rises.
        let mut generator = UuidV7Generator::new();
        let mut prev = generator.next(5_000, &mut random);
        for _ in 0..5_000 {
            let id = generator.next(5_000, &mut random);
            assert!(id > prev);
            prev = id;
        }
    }

    #[test]
    fn legacy_ids_rekey_to_one_v7_whoever_does_it() {
        let a = uuid_v7_from_legacy("6f0e4c1a-2b7d-4e59-9a83-5c1d2e3f4a5b", 1_700_000_000_000);
        let b = uuid_v7_from_legacy("6f0e4c1a-2b7d-4e59-9a83-5c1d2e3f4a5b", 1_700_000_000_000);
        assert_eq!(a, b);
        assert!(is_uuid_v7(&a));
    }

    #[test]
    fn uuid_v5_matches_rfc_example() {
        // RFC 4122 DNS namespace, "www.example.com" (Python uuid.uuid5).
        assert_eq!(
            uuid_v5("6ba7b810-9dad-11d1-80b4-00c04fd430c8", "www.example.com"),
            "2ed6657d-e927-568b-95e1-2665a8aea6a2"
        );
    }

    #[test]
    fn v4_ids_have_version_and_variant() {
        let id = uuid_v4(&mut SeededRandom::new(3));
        assert_eq!(&id[14..15], "4");
        assert!(matches!(&id[19..20], "8" | "9" | "a" | "b"));
    }
}
