//! Clock schedules with no geolocation and no prayer-time astronomy:
//!
//! - per-card recite slots (PARITY G6; web `lib/home/schedule.ts`): named
//!   `HH:MM` times, "due" inside an inclusive ±30-minute window;
//! - the daily goal reminder (PARITY G7; web `lib/reminders/schedule.ts`):
//!   one minute of the day plus optional quiet hours.
//!
//! Every function takes the minute of the local day explicitly, so callers
//! choose the zone and tests stay deterministic.

use serde_json::Value;

use crate::api::types::ReminderSchedule;

pub const MINUTES_PER_DAY: i64 = 24 * 60;
/// A slot is due inside an inclusive ±30-minute window around its time.
const DUE_WINDOW_MIN: i64 = 30;

/// One recite slot of a card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleSlot {
    pub id: String,
    pub label: String,
    /// Strict 24-hour `HH:MM`.
    pub time: String,
}

impl ScheduleSlot {
    pub fn to_json(&self) -> Value {
        serde_json::json!({ "id": self.id, "label": self.label, "time": self.time })
    }
}

/// The editor's named starting points (`SCHEDULE_PRESETS`): label, time.
pub const SCHEDULE_PRESETS: [(&str, &str); 7] = [
    ("Fajr", "05:00"),
    ("Dhuhr", "12:30"),
    ("Asr", "15:30"),
    ("Maghrib", "18:00"),
    ("Isha", "19:30"),
    ("Morning", "07:00"),
    ("Evening", "17:30"),
];

/// `HH:MM` → minute of the day, `None` for anything but a strict clock.
pub fn minutes_from_clock(value: &str) -> Option<i64> {
    let b = value.as_bytes();
    if b.len() != 5
        || b[2] != b':'
        || !b
            .iter()
            .enumerate()
            .all(|(i, c)| i == 2 || c.is_ascii_digit())
    {
        return None;
    }
    let hours: i64 = value[0..2].parse().ok()?;
    let minutes: i64 = value[3..5].parse().ok()?;
    (hours <= 23 && minutes <= 59).then_some(hours * 60 + minutes)
}

/// `parseTimeHHMM`: the time when it is a strict `HH:MM`.
pub fn parse_time_hhmm(raw: &str) -> Option<String> {
    minutes_from_clock(raw).map(|_| raw.to_owned())
}

/// Minute of the day → `HH:MM`, wrapping (`clockFromMinutes`).
pub fn clock_from_minutes(minute: i64) -> String {
    let safe = minute.rem_euclid(MINUTES_PER_DAY);
    format!("{:02}:{:02}", safe / 60, safe % 60)
}

/// A card's slots, decoded from its stored `schedule` array.
pub fn slots_of(card: &Value) -> Vec<ScheduleSlot> {
    card.get("schedule")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|s| {
            Some(ScheduleSlot {
                id: s.get("id")?.as_str()?.to_owned(),
                label: s.get("label")?.as_str()?.to_owned(),
                time: s.get("time")?.as_str()?.to_owned(),
            })
        })
        .collect()
}

/// The badge state of a schedule (`SlotState`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotState {
    Due(ScheduleSlot),
    Later {
        slot: ScheduleSlot,
        minutes_until: i64,
    },
}

/// `slotStateFor`: a slot inside the ±30-minute window wins (the earliest
/// occurrence when several overlap); otherwise the soonest upcoming slot,
/// which may wrap midnight. `None` for an empty schedule.
pub fn slot_state_for(schedule: &[ScheduleSlot], now_minute: i64) -> Option<SlotState> {
    let mut due: Option<(&ScheduleSlot, i64)> = None;
    let mut next: Option<(&ScheduleSlot, i64)> = None;
    for slot in schedule {
        let Some(at) = minutes_from_clock(&slot.time) else {
            continue;
        };
        let until = (at - now_minute).rem_euclid(MINUTES_PER_DAY);
        let offset = if until <= DUE_WINDOW_MIN {
            until
        } else {
            until - MINUTES_PER_DAY
        };
        if offset >= -DUE_WINDOW_MIN {
            if due.is_none_or(|(_, o)| offset < o) {
                due = Some((slot, offset));
            }
            continue;
        }
        if next.is_none_or(|(_, u)| until < u) {
            next = Some((slot, until));
        }
    }
    if let Some((slot, _)) = due {
        return Some(SlotState::Due(slot.clone()));
    }
    next.map(|(slot, minutes_until)| SlotState::Later {
        slot: slot.clone(),
        minutes_until,
    })
}

// --- goal reminder ---

/// The only reminder kind.
pub const REMINDER_KIND: &str = "goal";
/// 20:00.
pub const DEFAULT_REMINDER_MINUTE: i64 = 20 * 60;

/// Why a reminder schedule cannot be saved (`ReminderProblem`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReminderProblem {
    BadTime,
    QuietIncomplete,
    QuietSame,
    QuietRange,
    InsideQuiet,
}

/// `inQuietHours`: start-inclusive, end-exclusive, wrapping midnight.
pub fn in_quiet_hours(minute: i64, start: Option<i64>, end: Option<i64>) -> bool {
    let (Some(start), Some(end)) = (start, end) else {
        return false;
    };
    if start == end {
        return false;
    }
    if start < end {
        return minute >= start && minute < end;
    }
    minute >= start || minute < end
}

fn in_day(m: i64) -> bool {
    (0..MINUTES_PER_DAY).contains(&m)
}

/// `scheduleProblem`: the first problem, or `None`.
pub fn reminder_problem(
    reminder_minute: i64,
    quiet_start: Option<i64>,
    quiet_end: Option<i64>,
) -> Option<ReminderProblem> {
    if !in_day(reminder_minute) {
        return Some(ReminderProblem::BadTime);
    }
    let (start, end) = match (quiet_start, quiet_end) {
        (None, None) => return None,
        (Some(s), Some(e)) => (s, e),
        _ => return Some(ReminderProblem::QuietIncomplete),
    };
    if !in_day(start) || !in_day(end) {
        return Some(ReminderProblem::QuietRange);
    }
    if start == end {
        return Some(ReminderProblem::QuietSame);
    }
    in_quiet_hours(reminder_minute, quiet_start, quiet_end).then_some(ReminderProblem::InsideQuiet)
}

/// Who shows a schedule's daily reminder on this install.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReminderDelivery {
    /// Off, or held by its own quiet hours.
    None,
    /// The server's push (`services/reminders.rs`: the account's most
    /// recently seen device with a push token, once a day).
    Server,
    /// This install, at this minute of the day in the schedule's zone.
    Local(u16),
}

/// The server pushes the reminder to one of the account's devices with a
/// push token. An install that registered one is among them, so its own
/// notification yields: the user is told once.
pub fn reminder_delivery(schedule: &ReminderSchedule, push_token: bool) -> ReminderDelivery {
    let minute = schedule.reminder_minute.clamp(0, 1439);
    let quiet = in_quiet_hours(
        i64::from(minute),
        schedule.quiet_start_minute.map(i64::from),
        schedule.quiet_end_minute.map(i64::from),
    );
    if !schedule.enabled || quiet {
        return ReminderDelivery::None;
    }
    if push_token {
        return ReminderDelivery::Server;
    }
    ReminderDelivery::Local(u16::try_from(minute).unwrap_or(0))
}

/// Minutes from `now_minute` to the reminder's next occurrence (1..=1440:
/// a reminder due this very minute fires tomorrow).
pub fn minutes_until_reminder(reminder_minute: i64, now_minute: i64) -> i64 {
    match (reminder_minute - now_minute).rem_euclid(MINUTES_PER_DAY) {
        0 => MINUTES_PER_DAY,
        n => n,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(id: &str, time: &str) -> ScheduleSlot {
        ScheduleSlot {
            id: id.into(),
            label: id.into(),
            time: time.into(),
        }
    }

    #[test]
    fn due_window_and_midnight_wrap() {
        let s = [slot("a", "05:00"), slot("b", "23:50")];
        // 00:10: b was 20 min ago → due.
        assert_eq!(slot_state_for(&s, 10), Some(SlotState::Due(s[1].clone())));
        // 03:00: a in 120 min.
        assert_eq!(
            slot_state_for(&s, 180),
            Some(SlotState::Later {
                slot: s[0].clone(),
                minutes_until: 120
            })
        );
        assert_eq!(slot_state_for(&[], 0), None);
    }

    #[test]
    fn a_registered_push_token_leaves_the_reminder_to_the_server() {
        let mut s = ReminderSchedule {
            kind: REMINDER_KIND.into(),
            enabled: true,
            time_zone: "UTC".into(),
            reminder_minute: 1200,
            quiet_start_minute: None,
            quiet_end_minute: None,
            updated_at: None,
        };
        assert_eq!(reminder_delivery(&s, false), ReminderDelivery::Local(1200));
        assert_eq!(reminder_delivery(&s, true), ReminderDelivery::Server);
        s.quiet_start_minute = Some(1100);
        s.quiet_end_minute = Some(1300);
        assert_eq!(reminder_delivery(&s, false), ReminderDelivery::None);
        s.enabled = false;
        assert_eq!(reminder_delivery(&s, true), ReminderDelivery::None);
    }

    #[test]
    fn reminder_rules() {
        assert_eq!(minutes_from_clock("07:05"), Some(425));
        assert_eq!(minutes_from_clock("24:00"), None);
        assert_eq!(clock_from_minutes(-1), "23:59");
        assert!(in_quiet_hours(23 * 60, Some(22 * 60), Some(7 * 60)));
        assert!(!in_quiet_hours(7 * 60, Some(22 * 60), Some(7 * 60)));
        assert_eq!(
            reminder_problem(23 * 60, Some(22 * 60), Some(7 * 60)),
            Some(ReminderProblem::InsideQuiet)
        );
        assert_eq!(
            reminder_problem(1200, Some(1), None),
            Some(ReminderProblem::QuietIncomplete)
        );
        assert_eq!(reminder_problem(1200, None, None), None);
        assert_eq!(minutes_until_reminder(600, 600), MINUTES_PER_DAY);
        assert_eq!(minutes_until_reminder(600, 1380), 660);
    }
}
