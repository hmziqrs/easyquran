//! Optional offline audio (web `lib/audio/offline.ts`). The web keeps files
//! in the Cache API; here they are files under `${dataDir}/audio`, one per
//! URL, named by [`offline_file_name`]. A device-local index (same key and
//! shape as the web: `{ url: bytes }`) records each file's size so the
//! budget in [`super::policy`] holds without reading every file. The app
//! does the file I/O, off the engine thread.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use super::policy::{OFFLINE_AUDIO_MAX_FILE_BYTES, admit_offline_bytes};
use crate::kv::KeyValue;

/// `OFFLINE_AUDIO_INDEX_KEY`.
pub const OFFLINE_AUDIO_INDEX_KEY: &str = "easyquran.audio.offlineIndex.v1";
/// The folder under the app data dir (the web's `OFFLINE_AUDIO_CACHE`).
pub const OFFLINE_AUDIO_DIR: &str = "audio";

/// url -> stored bytes.
pub type OfflineIndex = BTreeMap<String, u64>;

/// `CacheResult`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheResult {
    Ok,
    /// The file would break the budget.
    Limit,
    Failed,
}

/// `readIndex`: entries outside 1..=20 MB are dropped.
pub fn parse_index(raw: &str) -> OfflineIndex {
    let Ok(Value::Object(obj)) = serde_json::from_str::<Value>(raw) else {
        return OfflineIndex::new();
    };
    obj.into_iter()
        .filter_map(|(url, bytes)| {
            let n = bytes.as_f64()?;
            let ok = n.is_finite() && n >= 1.0 && n <= OFFLINE_AUDIO_MAX_FILE_BYTES as f64;
            ok.then_some((url, n as u64))
        })
        .collect()
}

pub fn encode_index(index: &OfflineIndex) -> String {
    let obj: Map<String, Value> = index
        .iter()
        .map(|(url, bytes)| (url.clone(), Value::from(*bytes)))
        .collect();
    Value::Object(obj).to_string()
}

pub fn read_index(kv: &dyn KeyValue) -> OfflineIndex {
    kv.get(OFFLINE_AUDIO_INDEX_KEY)
        .map(|raw| parse_index(&raw))
        .unwrap_or_default()
}

/// `writeIndex`: `false` when storage refuses.
pub fn write_index(kv: &dyn KeyValue, index: &OfflineIndex) -> bool {
    kv.set(OFFLINE_AUDIO_INDEX_KEY, &encode_index(index))
        .is_ok()
}

/// `offlineBytesUsed`.
pub fn bytes_used(index: &OfflineIndex) -> u64 {
    index.values().sum()
}

/// Whether a downloaded file of `bytes` may be stored next to `index`.
pub fn admit(index: &OfflineIndex, bytes: u64) -> CacheResult {
    if admit_offline_bytes(bytes_used(index), bytes) {
        return CacheResult::Ok;
    }
    CacheResult::Limit
}

/// The file name a URL is stored under: ASCII letters, digits, `.` and `-`
/// kept, every other byte written `_XX` (hex), so two URLs never share a
/// name and no name holds a path separator.
pub fn offline_file_name(url: &str) -> String {
    let mut out = String::with_capacity(url.len() + 8);
    for b in url.bytes() {
        if b.is_ascii_alphanumeric() || b == b'.' || b == b'-' {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("_{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kv::MemoryKv;

    #[test]
    fn index_round_trips_and_filters() {
        let kv = MemoryKv::default();
        let mut index = OfflineIndex::new();
        index.insert("https://a/1.mp3".into(), 10);
        assert!(write_index(&kv, &index));
        assert_eq!(read_index(&kv), index);
        let bad = parse_index(r#"{"a":0,"b":99999999999,"c":"x","d":5}"#);
        assert_eq!(bad.into_iter().collect::<Vec<_>>(), [("d".to_owned(), 5)]);
    }

    #[test]
    fn budget() {
        let mut index = OfflineIndex::new();
        index.insert("a".into(), OFFLINE_AUDIO_MAX_FILE_BYTES);
        assert_eq!(admit(&index, 1), CacheResult::Ok);
        assert_eq!(
            admit(&index, OFFLINE_AUDIO_MAX_FILE_BYTES + 1),
            CacheResult::Limit
        );
    }

    #[test]
    fn file_names_are_flat_and_distinct() {
        let a = offline_file_name("https://h.org/001/001001.mp3");
        assert!(!a.contains('/') && !a.contains(':'));
        assert_ne!(offline_file_name("a/b"), offline_file_name("a_2Fb"));
        assert!(a.ends_with(".mp3"));
    }
}
