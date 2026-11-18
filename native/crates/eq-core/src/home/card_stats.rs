//! Per-card statistics from the recite event ring (PARITY G6; web
//! `lib/home/card-stats.ts`): calendar-window counts, streaks over local
//! days, last recite, a 30-day activity strip. Days are local
//! `YYYY-MM-DD` keys in a zone `offset_minutes` east of UTC; windows are
//! calendar days, never 24 h subtractions. `total` stays the card's
//! uncapped tally.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::store::goals::{add_days, days_between, local_day};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardStats {
    pub total: i64,
    pub today: i64,
    pub last7: i64,
    pub last30: i64,
    pub current_streak: i64,
    pub best_streak: i64,
    pub last_recited_at: Option<i64>,
    /// 30 local days ending today, oldest first, zero days included.
    pub activity30: Vec<(String, i64)>,
}

fn days_ending(today: &str, n: i64) -> Vec<String> {
    (0..n).rev().map(|i| add_days(today, -i)).collect()
}

/// Consecutive recite days back from today, or from yesterday while
/// today has none yet (the grace rule).
fn current_streak(days: &[&String], today: &str) -> i64 {
    let Some(last) = days.last() else {
        return 0;
    };
    if last.as_str() != today && last.as_str() != add_days(today, -1) {
        return 0;
    }
    let mut streak = 1;
    for pair in days.windows(2).rev() {
        if days_between(pair[0], pair[1]) != 1 {
            break;
        }
        streak += 1;
    }
    streak
}

fn best_streak(days: &[&String]) -> i64 {
    let (mut best, mut run) = (0, 0);
    let mut previous: Option<&String> = None;
    for day in days {
        run = match previous {
            Some(p) if days_between(p, day) == 1 => run + 1,
            _ => 1,
        };
        best = best.max(run);
        previous = Some(day);
    }
    best
}

/// Every statistic of `card` (its `count` and `events`) at `now_ms`.
pub fn card_stats(card: &Value, now_ms: i64, offset_minutes: i64) -> CardStats {
    let events: Vec<i64> = card
        .get("events")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_i64)
        .collect();
    let today = local_day(now_ms, offset_minutes);
    let mut counts: BTreeMap<String, i64> = BTreeMap::new();
    for ms in &events {
        *counts.entry(local_day(*ms, offset_minutes)).or_default() += 1;
    }
    let days: Vec<&String> = counts.keys().collect();
    let window = |n| -> i64 {
        days_ending(&today, n)
            .iter()
            .map(|d| counts.get(d).copied().unwrap_or(0))
            .sum()
    };
    CardStats {
        total: card.get("count").and_then(Value::as_i64).unwrap_or(0),
        today: counts.get(&today).copied().unwrap_or(0),
        last7: window(7),
        last30: window(30),
        current_streak: current_streak(&days, &today),
        best_streak: best_streak(&days),
        last_recited_at: events.iter().copied().max(),
        activity30: days_ending(&today, 30)
            .into_iter()
            .map(|d| {
                let n = counts.get(&d).copied().unwrap_or(0);
                (d, n)
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = 86_400_000;

    #[test]
    fn streaks_windows_and_grace() {
        // Local zone UTC+5; "now" is day 100, 10:00 local.
        let now = 100 * DAY + 5 * 3_600_000;
        let at = |day: i64| day * DAY + 3_600_000;
        // Days 95, 96 and 98, 99: yesterday ends a 2-day run (grace).
        let card = serde_json::json!({
            "count": 9, "events": [at(95), at(96), at(98), at(99), at(99)]
        });
        let s = card_stats(&card, now, 300);
        assert_eq!(s.total, 9);
        assert_eq!(s.today, 0);
        assert_eq!(s.last7, 5);
        assert_eq!(s.current_streak, 2);
        assert_eq!(s.best_streak, 2);
        assert_eq!(s.activity30.len(), 30);
        assert_eq!(s.activity30.last().map(|d| d.1), Some(0));
        let stale = serde_json::json!({ "count": 1, "events": [at(90)] });
        assert_eq!(card_stats(&stale, now, 300).current_streak, 0);
    }
}
