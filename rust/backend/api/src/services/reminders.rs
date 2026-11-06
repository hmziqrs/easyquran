//! Goal reminder decisions and the due-run loop (F9).
//!
//! A reminder fires once per local day, at the first scheduler tick at or after
//! its time in the user's zone, never inside quiet hours. It goes to the account's
//! most recently seen device with a push token only, so the user is told once.
//! Push text is generic: it names no account, goal or verse.

use std::future::Future;

use chrono::{DateTime, Timelike, Utc};
use chrono_tz::Tz;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};

use crate::db::sea_models::device;
use crate::db::sea_models::reminder_schedule::{self, Model};
use crate::error::{DbResult, DbResultExt};

pub const MINUTES_PER_DAY: i32 = 24 * 60;

/// Why a push did not reach a device. `Unregistered` prunes the device row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendError {
    Unregistered,
    Failed,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RunStats {
    pub considered: usize,
    pub sent: usize,
    pub skipped: usize,
    pub no_device: usize,
    pub failed: usize,
    pub pruned: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Due now. Carries the local day (`YYYY-MM-DD`) it belongs to.
    Fire(String),
    NotYet,
    Quiet,
    FiredToday,
    /// The stored zone no longer parses. Never fires.
    BadZone,
}

/// Quiet hours cover `[start, end)`. A window whose start is after its end wraps
/// midnight. Equal bounds mean no quiet hours.
pub fn in_quiet_hours(minute: i32, start: Option<i32>, end: Option<i32>) -> bool {
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

/// Local `(day, minute of day)` for an instant in `zone`.
pub fn local_clock(now: DateTime<Utc>, zone: Tz) -> (String, i32) {
    let local = now.with_timezone(&zone);
    let minute = (local.hour() * 60 + local.minute()) as i32;
    (local.format("%Y-%m-%d").to_string(), minute)
}

/// Checks a settings write before it is stored. Returns the first problem as a
/// short message, or `None` when the schedule is valid.
pub fn schedule_problem(
    time_zone: &str,
    reminder_minute: i32,
    quiet_start: Option<i32>,
    quiet_end: Option<i32>,
) -> Option<&'static str> {
    if time_zone.parse::<Tz>().is_err() {
        return Some("Unknown time zone");
    }
    if !(0..MINUTES_PER_DAY).contains(&reminder_minute) {
        return Some("Reminder time must be within one day");
    }
    match (quiet_start, quiet_end) {
        (None, None) => None,
        (Some(start), Some(end)) => {
            if !(0..MINUTES_PER_DAY).contains(&start) || !(0..MINUTES_PER_DAY).contains(&end) {
                return Some("Quiet hours must be within one day");
            }
            if start == end {
                return Some("Quiet hours need two different times");
            }
            if in_quiet_hours(reminder_minute, Some(start), Some(end)) {
                return Some("Reminder time falls inside quiet hours");
            }
            None
        }
        _ => Some("Set both quiet hour times or neither"),
    }
}

/// Whether the schedule is due at `now`. Pure: the caller supplies the clock.
pub fn decide(schedule: &Model, now: DateTime<Utc>) -> Decision {
    let Ok(zone) = schedule.time_zone.parse::<Tz>() else {
        return Decision::BadZone;
    };
    let (day, minute) = local_clock(now, zone);
    if schedule.last_fired_day.as_deref() == Some(day.as_str()) {
        return Decision::FiredToday;
    }
    if minute < schedule.reminder_minute {
        return Decision::NotYet;
    }
    if in_quiet_hours(
        minute,
        schedule.quiet_start_minute,
        schedule.quiet_end_minute,
    ) {
        return Decision::Quiet;
    }
    Decision::Fire(day)
}

/// Runs every enabled schedule that is due at `now`. `send` pushes to one device
/// token. Each row is decided alone, so one bad row never stops the others.
pub async fn run_due<F, Fut>(
    db: &DatabaseConnection,
    now: DateTime<Utc>,
    send: F,
) -> DbResult<RunStats>
where
    F: Fn(String) -> Fut,
    Fut: Future<Output = Result<(), SendError>>,
{
    let mut stats = RunStats::default();
    for schedule in reminder_schedule::Entity::list_enabled(db).await? {
        stats.considered += 1;
        let day = match decide(&schedule, now) {
            Decision::Fire(day) => day,
            _ => {
                stats.skipped += 1;
                continue;
            }
        };
        let newest = device::Entity::find()
            .filter(device::Column::UserId.eq(schedule.user_id))
            .order_by_desc(device::Column::LastSeenAt)
            .one(db)
            .await
            .map_err_to_response()?;
        let Some(newest) = newest else {
            stats.no_device += 1;
            continue;
        };
        let claimed =
            reminder_schedule::Entity::claim_day(db, schedule.user_id, &schedule.kind, &day)
                .await?;
        if claimed == 0 {
            stats.skipped += 1;
            continue;
        }
        match send(newest.token.clone()).await {
            Ok(()) => stats.sent += 1,
            Err(SendError::Unregistered) => {
                stats.pruned += 1;
                device::Entity::prune_owned_by_id(db, newest.id, schedule.user_id).await?;
                reminder_schedule::Entity::release_day(db, schedule.user_id, &schedule.kind, &day)
                    .await?;
            }
            Err(SendError::Failed) => {
                stats.failed += 1;
                reminder_schedule::Entity::release_day(db, schedule.user_id, &schedule.kind, &day)
                    .await?;
            }
        }
    }
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap()
    }

    fn schedule(zone: &str, reminder: i32, quiet: Option<(i32, i32)>) -> Model {
        Model {
            user_id: 1,
            kind: "goal".into(),
            enabled: true,
            time_zone: zone.into(),
            reminder_minute: reminder,
            quiet_start_minute: quiet.map(|q| q.0),
            quiet_end_minute: quiet.map(|q| q.1),
            last_fired_day: None,
            updated_at: chrono::Utc::now().fixed_offset(),
            expires_at: reminder_schedule::NEVER_EXPIRES,
            retention_class: reminder_schedule::RETENTION_CLASS.into(),
        }
    }

    #[test]
    fn quiet_window_wraps_midnight() {
        // 22:00 to 07:00
        assert!(in_quiet_hours(23 * 60, Some(22 * 60), Some(7 * 60)));
        assert!(in_quiet_hours(60, Some(22 * 60), Some(7 * 60)));
        assert!(!in_quiet_hours(8 * 60, Some(22 * 60), Some(7 * 60)));
        assert!(!in_quiet_hours(21 * 60 + 59, Some(22 * 60), Some(7 * 60)));
    }

    #[test]
    fn quiet_window_is_start_inclusive_end_exclusive() {
        assert!(in_quiet_hours(60, Some(60), Some(120)));
        assert!(!in_quiet_hours(120, Some(60), Some(120)));
    }

    #[test]
    fn equal_or_missing_quiet_bounds_mean_no_quiet_hours() {
        assert!(!in_quiet_hours(60, Some(60), Some(60)));
        assert!(!in_quiet_hours(60, None, None));
        assert!(!in_quiet_hours(60, Some(0), None));
    }

    #[test]
    fn fires_at_local_time_not_utc() {
        // 20:00 in Karachi (UTC+5) is 15:00 UTC.
        let s = schedule("Asia/Karachi", 20 * 60, None);
        assert_eq!(decide(&s, at(2026, 11, 6, 14, 59)), Decision::NotYet);
        assert_eq!(
            decide(&s, at(2026, 11, 6, 15, 0)),
            Decision::Fire("2026-11-06".into())
        );
    }

    #[test]
    fn local_day_rolls_over_in_the_users_zone() {
        // 23:30 UTC on the 6th is already the 7th in Karachi.
        let (day, minute) = local_clock(at(2026, 11, 6, 23, 30), "Asia/Karachi".parse().unwrap());
        assert_eq!(day, "2026-11-07");
        assert_eq!(minute, 4 * 60 + 30);
    }

    #[test]
    fn follows_daylight_saving_time() {
        // New York springs forward on 2026-03-08 (EST UTC-5 to EDT UTC-4). 07:00
        // local is 12:00 UTC on the 7th and 11:00 UTC on the 8th.
        let s = schedule("America/New_York", 7 * 60, None);
        assert_eq!(decide(&s, at(2026, 3, 7, 11, 59)), Decision::NotYet);
        assert_eq!(
            decide(&s, at(2026, 3, 7, 12, 0)),
            Decision::Fire("2026-03-07".into())
        );
        assert_eq!(decide(&s, at(2026, 3, 8, 10, 59)), Decision::NotYet);
        assert_eq!(
            decide(&s, at(2026, 3, 8, 11, 0)),
            Decision::Fire("2026-03-08".into())
        );
    }

    #[test]
    fn quiet_hours_hold_the_reminder() {
        // Reminder 08:00, quiet 07:30 to 09:00: at 08:00 it waits.
        let s = schedule("UTC", 8 * 60, Some((7 * 60 + 30, 9 * 60)));
        assert_eq!(decide(&s, at(2026, 1, 1, 8, 0)), Decision::Quiet);
    }

    #[test]
    fn fires_once_per_local_day() {
        let mut s = schedule("UTC", 9 * 60, None);
        s.last_fired_day = Some("2026-01-01".into());
        assert_eq!(decide(&s, at(2026, 1, 1, 12, 0)), Decision::FiredToday);
        assert_eq!(
            decide(&s, at(2026, 1, 2, 9, 0)),
            Decision::Fire("2026-01-02".into())
        );
    }

    #[test]
    fn unparseable_zone_never_fires() {
        let s = schedule("Not/AZone", 0, None);
        assert_eq!(decide(&s, at(2026, 1, 1, 12, 0)), Decision::BadZone);
    }

    #[test]
    fn problems_are_refused_before_storage() {
        assert!(schedule_problem("Nope/Zone", 0, None, None).is_some());
        assert!(schedule_problem("UTC", 1440, None, None).is_some());
        assert!(schedule_problem("UTC", 60, Some(0), None).is_some());
        assert!(schedule_problem("UTC", 60, Some(30), Some(30)).is_some());
        assert!(schedule_problem("UTC", 8 * 60, Some(7 * 60), Some(9 * 60)).is_some());
        assert!(schedule_problem("UTC", 20 * 60, Some(22 * 60), Some(7 * 60)).is_none());
        assert!(schedule_problem("UTC", 20 * 60, None, None).is_none());
    }
}
