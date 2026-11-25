//! Local notifications over NSH `notifications` (PARITY G7, M1); remote push
//! (M2) shows through here too ([`show_push_hint`]).
//!
//! [`schedule_daily_reminder`] hands the goal reminder to the system, so it
//! shows while the app is closed. NSH and its modules schedule none, so each
//! platform has its own part:
//!
//! - iOS and macOS, packaged: a repeating `UNCalendarNotificationTrigger`
//!   (`apple`);
//! - Android: an inexact alarm the app's `BroadcastReceiver` re-sets
//!   (`android`, `native/android/reminders`);
//! - Windows: toasts scheduled for the days ahead, where the app has an
//!   identity the system shows toasts for (`windows`);
//! - Linux has no scheduler for apps.
//!
//! Where it returns `false` (Linux, unbundled and development runs) the
//! in-app timer of `features::reminders` shows it while the app runs.

use eq_core::routes::{Link, SCHEME, StaticPage, link_path};
use nsh_core::{
    notifications::Notification,
    prelude::{next_notification_press, show_notification, spawn},
};

#[cfg(target_os = "android")]
mod android;
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod apple;
#[cfg(target_os = "macos")]
mod foreign;
#[cfg(target_os = "windows")]
mod windows;

/// The web's `PUSH_TITLE` / `PUSH_BODY` / `PUSH_TAG` (`push-privacy.ts`):
/// the only text a push ever shows, one notification replaced by the next.
// i18n: web constants in push-privacy.ts, not catalogue keys
const PUSH_TITLE: &str = "EasyQuran";
const PUSH_BODY: &str = "Open EasyQuran to view notifications.";
const PUSH_TAG: &str = "easyquran-notifications";

/// Shows the fixed public push text. Nothing of the FCM payload (account,
/// notification details) is read or shown (`publicPushPayload`).
pub async fn show_push_hint() -> bool {
    show_notification(Notification::new(PUSH_TAG, PUSH_TITLE, PUSH_BODY)).await
}

/// The goal reminder as the system shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyReminder {
    /// The notification's id: another with it replaces it.
    pub id: &'static str,
    pub title: String,
    pub body: String,
    /// Minute of the day in [`Self::zone`].
    pub minute: u16,
    /// The schedule's IANA zone; one the system does not know reads as the
    /// device's.
    pub zone: String,
    /// Quiet hours `[start, end)`: a late delivery inside them is dropped.
    pub quiet: Option<(u16, u16)>,
}

impl DailyReminder {
    #[cfg_attr(
        not(any(target_os = "macos", target_os = "ios", target_os = "windows")),
        allow(dead_code)
    )]
    fn clock(&self) -> (u8, u8) {
        let part = |n: u16| u8::try_from(n).unwrap_or(0);
        let minute = self.minute.min(1439);
        (part(minute / 60), part(minute % 60))
    }
}

/// The link a press opens where the notification carries one: the page the
/// web's does (`service-worker.ts`, `data.url`).
#[cfg_attr(
    not(any(target_os = "android", target_os = "windows")),
    allow(dead_code)
)]
fn press_link() -> String {
    let path = link_path(&Link::Page(StaticPage::Account)).unwrap_or_default();
    format!("{SCHEME}{}", path.trim_start_matches('/'))
}

/// Hands `reminder` to the system, in place of the one before. `true` when
/// the system shows it from here on, the app closed or not; `false` where
/// the caller keeps its in-app timer.
pub fn schedule_daily_reminder(reminder: &DailyReminder) -> bool {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        apple::schedule_daily(reminder)
    }
    #[cfg(target_os = "android")]
    {
        android::schedule_daily(reminder)
    }
    #[cfg(target_os = "windows")]
    {
        windows::schedule_daily(reminder)
    }
    #[cfg(not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "android",
        target_os = "windows"
    )))]
    {
        let _ = reminder;
        false
    }
}

/// Removes the daily reminder `id`, waiting or shown.
pub fn cancel_daily_reminder(id: &str) {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    apple::cancel(id);
    #[cfg(target_os = "android")]
    android::cancel(id);
    #[cfg(target_os = "windows")]
    windows::cancel(id);
    #[cfg(not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "android",
        target_os = "windows"
    )))]
    let _ = id;
}

/// Calls `open` for each press of one of the app's notifications, those that
/// started the app first. Once, from the root.
///
/// NSH reports the presses of the notifications it showed. The others (the
/// scheduled reminder, a remote push) go to the process's one foreign
/// handler: set here on macOS; nsh-extra's `push` on iOS and Android, whose
/// presses `stores::push` opens. On Android and Windows the scheduled
/// reminder carries [`press_link`], which arrives as a deep link instead.
pub fn listen_for_presses(open: impl Fn() + Copy + 'static) {
    spawn(async move {
        loop {
            let _ = next_notification_press().await;
            open();
        }
    });
    #[cfg(target_os = "macos")]
    {
        foreign::install();
        spawn(async move {
            while foreign::next_press().await {
                open();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unbundled_test_process_falls_back_safely_without_crashing() {
        assert!(!schedule_daily_reminder(&DailyReminder {
            id: "goal-reminder",
            title: "Goal reminder".into(),
            body: "Time to read".into(),
            minute: 8 * 60 + 30,
            zone: "UTC".into(),
            quiet: None,
        }));
        cancel_daily_reminder("goal-reminder");
    }
}
