//! Reading goals and streaks (PARITY G4; web `lib/goals/*`, contracts/sync.md
//! "Reading goals"). One document per owner, `{ "goals": [...] }`: signed
//! out the device's own, signed in the account's, synced whole as
//! `goals.reading`. Owners stay apart: one account's goals never land in
//! another's document. Progress is never stored: it is computed from day
//! totals. Days are device-local `YYYY-MM-DD` strings.

use serde_json::{Value, json};

use super::{
    AppStore, StoreError, preferences,
    rows::{int_in, kv_json, kv_set_json, opt_str, str_of},
};
use crate::sync::clock::{civil_from_days, days_from_civil};

/// Verses in the Quran: the target of a finish goal, counted in verse reads.
pub const QURAN_AYAH_TOTAL: i64 = 6236;
pub const GOAL_LIMIT: usize = 10;
pub const DAILY_MINUTES_MIN: i64 = 1;
pub const DAILY_MINUTES_MAX: i64 = 600;
/// Widest goal window, a year plus slack.
pub const GOAL_SPAN_MAX_DAYS: i64 = 400;
const DOC_KEY: &str = "goals.doc";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalKind {
    DailyMinutes,
    FinishQuran,
}

impl GoalKind {
    pub fn as_str(self) -> &'static str {
        match self {
            GoalKind::DailyMinutes => "daily_minutes",
            GoalKind::FinishQuran => "finish_quran",
        }
    }

    fn parse(raw: &str) -> Option<Self> {
        match raw {
            "daily_minutes" => Some(GoalKind::DailyMinutes),
            "finish_quran" => Some(GoalKind::FinishQuran),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadingGoal {
    pub id: String,
    pub kind: GoalKind,
    /// Minutes per day for a time goal; `None` for a finish goal.
    pub daily_minutes: Option<i64>,
    pub start_day: String,
    /// Last day of the window before pauses, inclusive.
    pub end_day: String,
    /// Day the goal was paused, or `None` while it runs.
    pub paused_from: Option<String>,
    /// Paused days already added to the window.
    pub paused_days: i64,
}

impl ReadingGoal {
    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id, "kind": self.kind.as_str(), "dailyMinutes": self.daily_minutes,
            "startDay": self.start_day, "endDay": self.end_day,
            "pausedFrom": self.paused_from, "pausedDays": self.paused_days,
        })
    }
}

/// One day from the server rollup (or this device's rollup signed out).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadingDay {
    pub local_day: String,
    pub ayahs_read: i64,
    pub ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalState {
    Active,
    Paused,
    Completed,
    Ended,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GoalProgress {
    pub done: f64,
    pub target: i64,
    pub state: GoalState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoalDraft {
    pub kind: GoalKind,
    pub daily_minutes: Option<i64>,
    pub start_day: String,
    pub end_day: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalDraftError {
    Dates,
    Span,
    Minutes,
    Limit,
}

/// `YYYY-MM-DD` → days since the epoch, `None` when not a real date.
fn day_number(day: &str) -> Option<i64> {
    let b = day.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let num = |r: std::ops::Range<usize>| -> Option<i64> {
        let part = day.get(r)?;
        part.bytes()
            .all(|c| c.is_ascii_digit())
            .then(|| part.parse().ok())?
    };
    let (y, m, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let n = days_from_civil(y, m as u32, d as u32);
    (civil_from_days(n) == (y, m as u32, d as u32)).then_some(n)
}

fn day_string(n: i64) -> String {
    let (y, m, d) = civil_from_days(n);
    format!("{y:04}-{m:02}-{d:02}")
}

pub fn is_day(day: &str) -> bool {
    day_number(day).is_some()
}

/// Whole days from `from` to `to` (negative when `to` is earlier); 0 for a bad day.
pub fn days_between(from: &str, to: &str) -> i64 {
    match (day_number(from), day_number(to)) {
        (Some(a), Some(b)) => b - a,
        _ => 0,
    }
}

pub fn add_days(day: &str, count: i64) -> String {
    day_number(day).map_or_else(|| day.to_owned(), |n| day_string(n + count))
}

/// The local day of an epoch ms in a zone `offset_minutes` east of UTC.
pub fn local_day(ms: i64, offset_minutes: i64) -> String {
    day_string((ms + offset_minutes * 60_000).div_euclid(86_400_000))
}

/// Minutes a day counts toward a time goal: a verse read always counts at least one minute.
pub fn credited_minutes(day: &ReadingDay) -> f64 {
    (day.ms as f64 / 60_000.0).max(day.ayahs_read as f64)
}

/// Last day of the window as of `today`: paused days never use up the window.
pub fn window_end(goal: &ReadingGoal, today: &str) -> String {
    let paused = goal
        .paused_from
        .as_deref()
        .map_or(0, |from| days_between(from, today).max(0));
    add_days(&goal.end_day, goal.paused_days + paused)
}

/// Total the goal asks for.
pub fn goal_target(goal: &ReadingGoal) -> i64 {
    match goal.kind {
        GoalKind::FinishQuran => QURAN_AYAH_TOTAL,
        GoalKind::DailyMinutes => {
            goal.daily_minutes.unwrap_or(0) * (days_between(&goal.start_day, &goal.end_day) + 1)
        }
    }
}

fn state_of(done: f64, target: i64, paused: bool, past_end: bool) -> GoalState {
    if done >= target as f64 {
        return GoalState::Completed;
    }
    if paused {
        return GoalState::Paused;
    }
    if past_end {
        return GoalState::Ended;
    }
    GoalState::Active
}

pub fn goal_progress(goal: &ReadingGoal, days: &[ReadingDay], today: &str) -> GoalProgress {
    let end = window_end(goal, today);
    let done: f64 = days
        .iter()
        .filter(|d| d.local_day.as_str() >= goal.start_day.as_str() && d.local_day <= end)
        .map(|d| match goal.kind {
            GoalKind::FinishQuran => d.ayahs_read as f64,
            GoalKind::DailyMinutes => credited_minutes(d),
        })
        .sum();
    let target = goal_target(goal);
    GoalProgress {
        done,
        target,
        state: state_of(
            done,
            target,
            goal.paused_from.is_some(),
            today > end.as_str(),
        ),
    }
}

/// Consecutive days with a verse read, back from today (or from yesterday while today has none).
pub fn streak_of(days: &[ReadingDay], today: &str) -> i64 {
    let read: std::collections::HashSet<&str> = days
        .iter()
        .filter(|d| d.ayahs_read > 0)
        .map(|d| d.local_day.as_str())
        .collect();
    let mut cursor = if read.contains(today) {
        today.to_owned()
    } else {
        add_days(today, -1)
    };
    let mut count = 0;
    while read.contains(cursor.as_str()) {
        count += 1;
        cursor = add_days(&cursor, -1);
    }
    count
}

/// The `weeks * 7` days ending today, oldest first.
pub fn calendar_days(today: &str, weeks: i64) -> Vec<String> {
    (0..weeks * 7)
        .rev()
        .map(|back| add_days(today, -back))
        .collect()
}

/// Checks a goal before it is stored: the first problem, or `None`.
pub fn draft_error(draft: &GoalDraft, existing: usize) -> Option<GoalDraftError> {
    if existing >= GOAL_LIMIT {
        return Some(GoalDraftError::Limit);
    }
    if !is_day(&draft.start_day) || !is_day(&draft.end_day) {
        return Some(GoalDraftError::Dates);
    }
    let span = days_between(&draft.start_day, &draft.end_day);
    if span < 0 {
        return Some(GoalDraftError::Dates);
    }
    if span + 1 > GOAL_SPAN_MAX_DAYS {
        return Some(GoalDraftError::Span);
    }
    let minutes_ok = draft
        .daily_minutes
        .is_some_and(|m| (DAILY_MINUTES_MIN..=DAILY_MINUTES_MAX).contains(&m));
    if draft.kind == GoalKind::DailyMinutes && !minutes_ok {
        return Some(GoalDraftError::Minutes);
    }
    None
}

pub fn goal_from_draft(draft: &GoalDraft, id: String) -> ReadingGoal {
    ReadingGoal {
        id,
        kind: draft.kind,
        daily_minutes: match draft.kind {
            GoalKind::DailyMinutes => draft.daily_minutes,
            GoalKind::FinishQuran => None,
        },
        start_day: draft.start_day.clone(),
        end_day: draft.end_day.clone(),
        paused_from: None,
        paused_days: 0,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalPreset {
    Minutes30,
    Finish30,
    Finish365,
}

/// 15 minutes a day for 30 days, finish in 30 days, finish in a year.
pub fn preset_draft(preset: GoalPreset, today: &str) -> GoalDraft {
    let (kind, minutes, span) = match preset {
        GoalPreset::Minutes30 => (GoalKind::DailyMinutes, Some(15), 30),
        GoalPreset::Finish30 => (GoalKind::FinishQuran, None, 30),
        GoalPreset::Finish365 => (GoalKind::FinishQuran, None, 365),
    };
    GoalDraft {
        kind,
        daily_minutes: minutes,
        start_day: today.to_owned(),
        end_day: add_days(today, span - 1),
    }
}

fn decode_goal(v: &Value) -> Option<ReadingGoal> {
    let id = str_of(v, "id").filter(|id| id.len() == 36)?;
    let kind = GoalKind::parse(str_of(v, "kind")?)?;
    let paused_from = match v.get("pausedFrom") {
        Some(Value::Null) => None,
        _ => Some(opt_str(v, "pausedFrom").filter(|d| is_day(d))?),
    };
    let paused_days = int_in(v, "pausedDays", 0, GOAL_SPAN_MAX_DAYS)?;
    let draft = GoalDraft {
        kind,
        daily_minutes: int_in(v, "dailyMinutes", DAILY_MINUTES_MIN, DAILY_MINUTES_MAX),
        start_day: str_of(v, "startDay").unwrap_or_default().to_owned(),
        end_day: str_of(v, "endDay").unwrap_or_default().to_owned(),
    };
    if draft_error(&draft, 0).is_some() {
        return None;
    }
    Some(ReadingGoal {
        paused_from,
        paused_days,
        ..goal_from_draft(&draft, id.to_owned())
    })
}

/// Forward-compatible decode of `{goals: [...]}`: bad rows drop, the limit holds.
pub fn decode_goals(raw: &Value) -> Vec<ReadingGoal> {
    let mut out: Vec<ReadingGoal> = Vec::new();
    for row in raw
        .get("goals")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(goal) = decode_goal(row) else {
            continue;
        };
        if out.iter().any(|g| g.id == goal.id) {
            continue;
        }
        out.push(goal);
        if out.len() >= GOAL_LIMIT {
            break;
        }
    }
    out
}

// --- store ---

/// The current owner's goals.
pub fn entries(store: &AppStore) -> Result<Vec<ReadingGoal>, StoreError> {
    Ok(kv_json(store.owner_conn(), DOC_KEY)?.map_or_else(Vec::new, |doc| decode_goals(&doc)))
}

/// The document as synced: `{ "goals": [...] }`.
pub(crate) fn document(store: &AppStore) -> Result<Value, StoreError> {
    let goals: Vec<Value> = entries(store)?.iter().map(ReadingGoal::to_json).collect();
    Ok(json!({ "goals": goals }))
}

fn write(store: &mut AppStore, goals: &[ReadingGoal]) -> Result<(), StoreError> {
    let doc = json!({ "goals": goals.iter().map(ReadingGoal::to_json).collect::<Vec<_>>() });
    let result = kv_set_json(store.writable()?, DOC_KEY, &doc);
    store.track(result)
}

fn write_and_capture(store: &mut AppStore, goals: &[ReadingGoal]) -> Result<(), StoreError> {
    write(store, goals)?;
    preferences::capture_documents(store)
}

/// A document that arrived from another device for this account.
pub(crate) fn replace(store: &mut AppStore, raw: &Value) -> Result<(), StoreError> {
    write(store, &decode_goals(raw))
}

/// Adds a goal: the first problem with the draft, or `None` when stored.
pub fn add(store: &mut AppStore, draft: &GoalDraft) -> Result<Option<GoalDraftError>, StoreError> {
    let mut goals = entries(store)?;
    if let Some(problem) = draft_error(draft, goals.len()) {
        return Ok(Some(problem));
    }
    let id = store.new_id();
    goals.push(goal_from_draft(draft, id));
    write_and_capture(store, &goals)?;
    Ok(None)
}

fn patch(
    store: &mut AppStore,
    id: &str,
    change: impl Fn(&mut ReadingGoal),
) -> Result<(), StoreError> {
    let mut goals = entries(store)?;
    goals.iter_mut().filter(|g| g.id == id).for_each(change);
    write_and_capture(store, &goals)
}

/// Pauses a running goal from `today`.
pub fn pause(store: &mut AppStore, id: &str, today: &str) -> Result<(), StoreError> {
    patch(store, id, |g| {
        if g.paused_from.is_none() {
            g.paused_from = Some(today.to_owned());
        }
    })
}

/// Resumes a paused goal, extending its window by the days it was paused.
pub fn resume(store: &mut AppStore, id: &str, today: &str) -> Result<(), StoreError> {
    patch(store, id, |g| {
        if let Some(from) = g.paused_from.take() {
            g.paused_days += days_between(&from, today).max(0);
        }
    })
}

pub fn remove(store: &mut AppStore, id: &str) -> Result<(), StoreError> {
    let goals: Vec<ReadingGoal> = entries(store)?.into_iter().filter(|g| g.id != id).collect();
    write_and_capture(store, &goals)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn goal(kind: GoalKind, minutes: Option<i64>) -> ReadingGoal {
        goal_from_draft(
            &GoalDraft {
                kind,
                daily_minutes: minutes,
                start_day: "2026-11-01".into(),
                end_day: "2026-11-10".into(),
            },
            "019a0000-0000-7000-8000-000000000001".into(),
        )
    }

    #[test]
    fn days_and_windows() {
        assert_eq!(days_between("2026-02-28", "2026-03-01"), 1);
        assert_eq!(add_days("2024-02-28", 1), "2024-02-29");
        assert!(!is_day("2026-02-30"));
        let mut g = goal(GoalKind::DailyMinutes, Some(10));
        assert_eq!(goal_target(&g), 100);
        g.paused_from = Some("2026-11-05".into());
        assert_eq!(window_end(&g, "2026-11-08"), "2026-11-13");
        // Resume keeps the end the same before and after.
        g.paused_days += days_between("2026-11-05", "2026-11-08");
        g.paused_from = None;
        assert_eq!(window_end(&g, "2026-11-08"), "2026-11-13");
    }

    #[test]
    fn credit_streak_and_progress() {
        let day = |d: &str, ayahs, ms| ReadingDay {
            local_day: d.into(),
            ayahs_read: ayahs,
            ms,
        };
        assert_eq!(credited_minutes(&day("x", 5, 60_000)), 5.0);
        let days = [
            day("2026-11-01", 1, 0),
            day("2026-11-02", 3, 0),
            day("2026-11-03", 0, 0),
        ];
        assert_eq!(streak_of(&days, "2026-11-03"), 2);
        let p = goal_progress(&goal(GoalKind::FinishQuran, None), &days, "2026-11-03");
        assert_eq!(
            (p.done, p.target, p.state),
            (4.0, QURAN_AYAH_TOTAL, GoalState::Active)
        );
        assert_eq!(
            draft_error(&preset_draft(GoalPreset::Finish365, "2026-11-01"), 0),
            None
        );
    }

    #[test]
    fn decode_drops_bad_rows_and_holds_the_limit() {
        let good = goal(GoalKind::DailyMinutes, Some(5)).to_json();
        let raw = json!({ "goals": [good, good, {"id": "x"}, {"kind": "finish_quran"}] });
        assert_eq!(decode_goals(&raw).len(), 1);
    }
}
