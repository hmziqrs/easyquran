//! Reading insights (PARITY G5; web `lib/analytics/reading-analytics.ts`).
//! Always on: these are the reader's own numbers, not product analytics.
//!
//! Every counted read becomes one event with a fresh UUID v7, queued per
//! owner in the key-value store (`easyquran.analytics.queue.<owner>`, capped
//! at 2000). Signed out, the owner is `local` and each event is also folded
//! into a 60-day rollup (`easyquran.analytics.local`) that answers the
//! summary and the goals' day totals on this device. Signed in, the queue
//! flushes 4 s after the last read, 100 events a batch, to
//! `user/analytics/v1/events`; the server inserts each id once, so a retry
//! never double-counts. A 400 drops the batch; anything else keeps it. On
//! the first flush after sign-in the signed-out queue joins the account's
//! (ids de-duplicated) and the local rollup is cleared.

use std::{
    collections::BTreeMap,
    hash::{BuildHasher, Hasher},
    sync::atomic::{AtomicU16, Ordering},
    time::Duration,
};

use eq_core::{
    api::types::{
        AnalyticsSummary, AnalyticsTotals, AyahTotal, DayTotal, ReadingEvent, SurahTotal,
    },
    kv::KeyValue,
    store::goals::ReadingDay,
    sync::{clock::iso_from_ms, ids::encode_uuid_v7},
};
use nsh_core::prelude::*;
use serde_json::{Value, json};

use crate::{app::auth_store, services::storage::NshKv, stores::home};

const ROLLUP_KEY: &str = "easyquran.analytics.local";
const QUEUE_KEY_PREFIX: &str = "easyquran.analytics.queue.";
const LOCAL_OWNER: &str = "local";
const BATCH_MAX: usize = 100;
const QUEUE_CAP: usize = 2000;
const ROLLUP_DAY_CAP: usize = 60;
const MS_MAX: i64 = 3_600_000;
const FLUSH_DELAY: Duration = Duration::from_secs(4);

/// One counted read, as the reader hands it over (`ReadInput`).
#[derive(Debug, Clone, PartialEq)]
pub struct ReadInput {
    pub session_id: Option<String>,
    pub surah: u16,
    pub ayah: u16,
    pub source_id: Option<String>,
    pub ms: f64,
}

fn queue_key(owner: &str) -> String {
    format!("{QUEUE_KEY_PREFIX}{owner}")
}

fn current_owner() -> String {
    auth_store()
        .user()
        .map_or_else(|| LOCAL_OWNER.to_owned(), |u| u.id.to_string())
}

fn source_ok(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
        })
}

fn event_json(e: &ReadingEvent) -> Value {
    serde_json::to_value(e).unwrap_or(Value::Null)
}

fn decode_event(v: &Value) -> Option<ReadingEvent> {
    let s = |k: &str| v.get(k).and_then(Value::as_str);
    let n = |k: &str, max: i64| {
        v.get(k)
            .and_then(Value::as_i64)
            .filter(|x| (0..=max).contains(x))
    };
    Some(ReadingEvent {
        id: s("id")?.to_owned(),
        session_id: s("sessionId").filter(|x| x.len() == 36).map(str::to_owned),
        surah: i32::try_from(n("surah", 114).filter(|x| *x >= 1)?).ok()?,
        ayah: i32::try_from(n("ayah", 286).filter(|x| *x >= 1)?).ok()?,
        source_id: s("sourceId").filter(|x| source_ok(x)).map(str::to_owned),
        ms: n("ms", MS_MAX)?,
        local_day: s("localDay")?.to_owned(),
        at: s("at")?.to_owned(),
    })
}

fn read_queue(owner: &str) -> Vec<ReadingEvent> {
    NshKv
        .get(&queue_key(owner))
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        .and_then(|doc| doc.get("events").and_then(Value::as_array).cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(decode_event)
        .collect()
}

fn write_queue(owner: &str, events: &[ReadingEvent]) {
    if events.is_empty() {
        NshKv.remove(&queue_key(owner));
        return;
    }
    let skip = events.len().saturating_sub(QUEUE_CAP);
    let doc =
        json!({ "v": 2, "events": events[skip..].iter().map(event_json).collect::<Vec<_>>() });
    if let Err(e) = NshKv.set(&queue_key(owner), &doc.to_string()) {
        log::warn!("insights: queue not saved: {e}");
    }
}

/// Day → verse key → (reads, ms).
type Rollup = BTreeMap<String, BTreeMap<String, (i64, i64)>>;

fn read_rollup() -> Rollup {
    let mut out = Rollup::new();
    let Some(doc) = NshKv
        .get(ROLLUP_KEY)
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        .filter(|d| d.get("v").and_then(Value::as_i64) == Some(2))
    else {
        return out;
    };
    for (day, verses) in doc
        .get("days")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
    {
        if !eq_core::store::goals::is_day(day) {
            continue;
        }
        let entry = out.entry(day.clone()).or_default();
        for (key, pair) in verses.as_object().into_iter().flatten() {
            let reads = pair
                .get(0)
                .and_then(Value::as_i64)
                .filter(|r| (1..=1_000_000).contains(r));
            let ms = pair.get(1).and_then(Value::as_i64).filter(|m| *m >= 0);
            if let (Some(reads), Some(ms)) = (reads, ms) {
                entry.insert(key.clone(), (reads, ms));
            }
        }
    }
    out
}

fn record_local(event: &ReadingEvent) {
    let mut rollup = read_rollup();
    let day = rollup.entry(event.local_day.clone()).or_default();
    let slot = day
        .entry(format!("{}:{}", event.surah, event.ayah))
        .or_default();
    slot.0 += 1;
    slot.1 += event.ms;
    let skip = rollup.len().saturating_sub(ROLLUP_DAY_CAP);
    let days: serde_json::Map<String, Value> = rollup
        .into_iter()
        .skip(skip)
        .map(|(d, verses)| {
            let v: serde_json::Map<String, Value> = verses
                .into_iter()
                .map(|(k, (r, m))| (k, json!([r, m])))
                .collect();
            (d, Value::Object(v))
        })
        .collect();
    let doc = json!({ "v": 2, "days": days });
    if let Err(e) = NshKv.set(ROLLUP_KEY, &doc.to_string()) {
        log::warn!("insights: rollup not saved: {e}");
    }
}

fn new_event_id(ms: i64) -> String {
    static COUNTER: AtomicU16 = AtomicU16::new(0);
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_i64(ms);
    let tail = h.finish().to_le_bytes();
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed) & 0x0fff;
    encode_uuid_v7(u64::try_from(ms).unwrap_or(0), counter, tail)
}

/// Queues one counted read (`readingAnalytics.emit`).
pub fn emit(input: ReadInput) {
    if !(1..=114).contains(&input.surah) || !(1..=286).contains(&input.ayah) {
        return;
    }
    let now = home::now_ms();
    #[allow(clippy::cast_possible_truncation)]
    let ms = (input.ms.round() as i64).clamp(0, MS_MAX);
    let event = ReadingEvent {
        id: new_event_id(now),
        session_id: input.session_id.filter(|s| s.len() == 36),
        surah: i32::from(input.surah),
        ayah: i32::from(input.ayah),
        source_id: input.source_id.filter(|s| source_ok(s)),
        ms,
        local_day: home::today(),
        at: iso_from_ms(now),
    };
    let owner = current_owner();
    let mut queue = read_queue(&owner);
    queue.push(event.clone());
    write_queue(&owner, &queue);
    if owner == LOCAL_OWNER {
        record_local(&event);
        return;
    }
    schedule_flush();
}

/// The latest scheduled flush: a newer read restarts the 4 s wait.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct FlushGeneration(u64);

fn schedule_flush() {
    let slot = global::<FlushGeneration>();
    let generation = slot.get_untracked().0 + 1;
    slot.set(FlushGeneration(generation));
    let _ = set_timeout(FLUSH_DELAY, move || {
        if global::<FlushGeneration>().get_untracked().0 == generation {
            let _ = spawn(flush_now());
        }
    });
}

fn absorb_local(owner: &str) {
    let local = read_queue(LOCAL_OWNER);
    if local.is_empty() {
        return;
    }
    let mut mine = read_queue(owner);
    let seen: std::collections::HashSet<String> = mine.iter().map(|e| e.id.clone()).collect();
    mine.extend(local.into_iter().filter(|e| !seen.contains(&e.id)));
    write_queue(owner, &mine);
    NshKv.remove(&queue_key(LOCAL_OWNER));
    NshKv.remove(ROLLUP_KEY);
}

/// Sends the account's queue, one batch (`flushNow`); more batches follow.
pub async fn flush_now() {
    let auth = auth_store();
    let (Some(user), Some(client)) = (auth.user(), auth.client()) else {
        return;
    };
    let owner = user.id.to_string();
    absorb_local(&owner);
    let queued = read_queue(&owner);
    if queued.is_empty() {
        return;
    }
    let batch: Vec<ReadingEvent> = queued.into_iter().take(BATCH_MAX).collect();
    let done = match client.analytics_events(user.id, &batch).await {
        Ok(_) => true,
        Err(e) if e.status() == 400 => {
            log::warn!("insights: dropped a rejected event batch");
            true
        }
        Err(_) => false,
    };
    if !done {
        return;
    }
    let sent: std::collections::HashSet<&str> = batch.iter().map(|e| e.id.as_str()).collect();
    let rest: Vec<ReadingEvent> = read_queue(&owner)
        .into_iter()
        .filter(|e| !sent.contains(e.id.as_str()))
        .collect();
    write_queue(&owner, &rest);
    if !rest.is_empty() {
        schedule_flush();
    }
}

/// `summarizeRollup`: the signed-out summary.
fn summarize(rollup: &Rollup) -> AnalyticsSummary {
    let mut per_surah: BTreeMap<i64, (i64, i64)> = BTreeMap::new();
    let mut per_ayah: BTreeMap<(i64, i64), (i64, i64)> = BTreeMap::new();
    let mut days = Vec::new();
    let (mut ayahs, mut total_ms) = (0, 0);
    for (day, verses) in rollup {
        let (mut reads_day, mut ms_day) = (0, 0);
        for (key, (reads, ms)) in verses {
            let Some((s, a)) = key.split_once(':') else {
                continue;
            };
            let (s, a) = (s.parse().unwrap_or(0), a.parse().unwrap_or(0));
            reads_day += reads;
            ms_day += ms;
            let e = per_surah.entry(s).or_default();
            *e = (e.0 + reads, e.1 + ms);
            let e = per_ayah.entry((s, a)).or_default();
            *e = (e.0 + reads, e.1 + ms);
        }
        ayahs += reads_day;
        total_ms += ms_day;
        days.push(DayTotal {
            local_day: day.clone(),
            ayahs_read: reads_day,
            ms: ms_day,
        });
    }
    let mut surahs: Vec<_> = per_surah.into_iter().collect();
    surahs.sort_by(|a, b| b.1.0.cmp(&a.1.0).then(a.0.cmp(&b.0)));
    let mut verses: Vec<_> = per_ayah.into_iter().collect();
    verses.sort_by(|a, b| {
        b.1.0
            .cmp(&a.1.0)
            .then_with(|| format!("{}:{}", a.0.0, a.0.1).cmp(&format!("{}:{}", b.0.0, b.0.1)))
    });
    days.sort_by(|a, b| b.local_day.cmp(&a.local_day));
    days.truncate(30);
    let day_count = i64::try_from(rollup.len()).unwrap_or(0);
    AnalyticsSummary {
        totals: AnalyticsTotals {
            ayahs,
            ms: total_ms,
            days: day_count,
        },
        top_surahs: surahs
            .into_iter()
            .take(5)
            .map(|(surah, (reads, ms))| SurahTotal { surah, reads, ms })
            .collect(),
        top_ayahs: verses
            .into_iter()
            .take(5)
            .map(|((surah, ayah), (reads, ms))| AyahTotal {
                surah,
                ayah,
                reads,
                ms,
            })
            .collect(),
        days,
    }
}

/// The summary: this device's rollup signed out, else the server's after a
/// flush. `None` when the server could not answer.
pub async fn fetch_summary() -> Option<AnalyticsSummary> {
    let auth = auth_store();
    let Some(client) = auth.client().filter(|_| auth.user().is_some()) else {
        return Some(summarize(&read_rollup()));
    };
    flush_now().await;
    client.analytics_summary().await.ok()
}

/// Day totals `from..=to`: the rollup signed out, else the server's.
pub async fn fetch_days(from: String, to: String) -> Option<Vec<ReadingDay>> {
    let auth = auth_store();
    let Some(client) = auth.client().filter(|_| auth.user().is_some()) else {
        let days = read_rollup()
            .into_iter()
            .filter(|(d, _)| *d >= from && *d <= to)
            .map(|(local_day, verses)| ReadingDay {
                local_day,
                ayahs_read: verses.values().map(|v| v.0).sum(),
                ms: verses.values().map(|v| v.1).sum(),
            })
            .collect();
        return Some(days);
    };
    flush_now().await;
    client.analytics_days(&from, &to).await.ok().map(|days| {
        days.into_iter()
            .map(|d| ReadingDay {
                local_day: d.local_day,
                ayahs_read: d.ayahs_read,
                ms: d.ms,
            })
            .collect()
    })
}
