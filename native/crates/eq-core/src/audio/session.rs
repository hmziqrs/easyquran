//! The saved player session (web `engine.svelte.ts`: `AUDIO_SESSION_KEY`,
//! `parseSession`, `#persist`): the queue is restored paused on launch,
//! never autoplayed.

use serde_json::{Value, json};

use super::{
    policy::nearest_rate,
    queue::{MAX_QUEUE_ITEMS, QueueSegment, is_valid_segment},
};
use crate::kv::KeyValue;

/// `AUDIO_SESSION_KEY`.
pub const AUDIO_SESSION_KEY: &str = "easyquran.audio.session.v1";

/// `SavedSession`.
#[derive(Debug, Clone, PartialEq)]
pub struct SavedSession {
    pub recording_id: String,
    pub segments: Vec<QueueSegment>,
    pub index: usize,
    pub rate: f32,
}

fn int_in(v: &Value, key: &str, min: f64, max: f64) -> Option<f64> {
    v.get(key)?
        .as_f64()
        .filter(|n| n.is_finite() && *n >= min && *n <= max)
}

fn decode_segment(v: &Value) -> Option<QueueSegment> {
    let s = QueueSegment {
        surah: int_in(v, "surah", 1.0, 114.0)? as u16,
        from: int_in(v, "from", 1.0, 286.0)? as u16,
        to: int_in(v, "to", 1.0, 286.0)? as u16,
    };
    is_valid_segment(s).then_some(s)
}

/// `parseSession`.
pub fn parse_session(raw: &str) -> Option<SavedSession> {
    let v: Value = serde_json::from_str(raw).ok()?;
    let recording_id = v.get("recordingId")?.as_str()?.to_owned();
    let index = int_in(&v, "index", 0.0, MAX_QUEUE_ITEMS as f64)? as usize;
    let rate = int_in(&v, "rate", 0.0, 2.0)?;
    let segments = v
        .get("segments")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(decode_segment).collect())
        .unwrap_or_default();
    Some(SavedSession {
        recording_id,
        segments,
        index,
        rate: nearest_rate(rate),
    })
}

pub fn encode_session(s: &SavedSession) -> String {
    let segments: Vec<Value> = s
        .segments
        .iter()
        .map(|g| json!({ "surah": g.surah, "from": g.from, "to": g.to }))
        .collect();
    json!({
        "recordingId": s.recording_id,
        "segments": segments,
        "index": s.index,
        "rate": s.rate,
    })
    .to_string()
}

pub fn load_session(kv: &dyn KeyValue) -> Option<SavedSession> {
    parse_session(&kv.get(AUDIO_SESSION_KEY)?)
}

/// `#persist` (a refused write is ignored, as the web's `tryWriteJSON`).
pub fn save_session(kv: &dyn KeyValue, s: &SavedSession) {
    let _ = kv.set(AUDIO_SESSION_KEY, &encode_session(s));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_rejects() {
        let s = SavedSession {
            recording_id: "r".into(),
            segments: vec![QueueSegment {
                surah: 1,
                from: 1,
                to: 7,
            }],
            index: 3,
            rate: 1.25,
        };
        assert_eq!(parse_session(&encode_session(&s)), Some(s));
        assert!(parse_session(r#"{"recordingId":"r","segments":[],"index":1,"rate":3}"#).is_none());
        let dropped = parse_session(
            r#"{"recordingId":"r","segments":[{"surah":0,"from":1,"to":2}],"index":0,"rate":1}"#,
        )
        .unwrap();
        assert!(dropped.segments.is_empty());
    }
}
