//! Local reminder notifications over NSH `notifications` (PARITY G7, M1);
//! remote push (M2) shows through here too ([`show_push_hint`]).
//!
//! On Apple platforms (iOS and macOS packaged with a bundle identifier),
//! [`schedule_daily_reminder`] schedules a repeating `UNCalendarNotificationTrigger`
//! with `UNUserNotificationCenter` so goal reminders fire even while the app
//! is closed. Unbundled runs (bare `cargo run`, headless tests) and other
//! platforms return `false` and fall back to the in-app timer in `reminders`.

use nsh_core::{notifications::Notification, prelude::show_notification};

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

/// Schedules a repeating daily OS notification at `hour:minute` local time.
/// Returns `true` when the OS scheduler accepted the request (packaged iOS /
/// macOS builds); `false` elsewhere so the caller keeps its in-app timer.
pub fn schedule_daily_reminder(id: &str, title: &str, body: &str, hour: u8, minute: u8) -> bool {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        apple::schedule_daily(id, title, body, hour, minute)
    }
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    {
        let _ = (id, title, body, hour, minute);
        false
    }
}

/// Removes any pending or delivered daily reminder with `id`.
pub fn cancel_daily_reminder(id: &str) {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        apple::cancel(id);
    }
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    {
        let _ = id;
    }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod apple {
    use std::{ffi::CStr, sync::OnceLock};

    use block2::RcBlock;
    use objc2::{
        msg_send,
        rc::Retained,
        runtime::{AnyClass, AnyObject, Bool},
    };
    use objc2_foundation::{NSArray, NSBundle, NSDateComponents, NSError, NSString};

    fn framework() -> bool {
        static LOADED: OnceLock<bool> = OnceLock::new();
        *LOADED.get_or_init(|| {
            let path: &CStr =
                c"/System/Library/Frameworks/UserNotifications.framework/UserNotifications";
            // SAFETY: dynamically loads the system framework; handle stays open.
            !unsafe { libc::dlopen(path.as_ptr(), libc::RTLD_LAZY) }.is_null()
        })
    }

    /// The current notification center, only when running in a real bundle.
    /// `UNUserNotificationCenter` terminates an unbundled executable that touches it.
    fn center() -> Option<Retained<AnyObject>> {
        if !framework() || NSBundle::mainBundle().bundleIdentifier().is_none() {
            return None;
        }
        let class = AnyClass::get(c"UNUserNotificationCenter")?;
        // SAFETY: documented class method, guarded by bundleIdentifier check.
        Some(unsafe { msg_send![class, currentNotificationCenter] })
    }

    pub fn schedule_daily(id: &str, title: &str, body: &str, hour: u8, minute: u8) -> bool {
        let Some(center) = center() else {
            return false;
        };
        let (Some(content_cls), Some(trigger_cls), Some(request_cls), Some(sound_cls)) = (
            AnyClass::get(c"UNMutableNotificationContent"),
            AnyClass::get(c"UNCalendarNotificationTrigger"),
            AnyClass::get(c"UNNotificationRequest"),
            AnyClass::get(c"UNNotificationSound"),
        ) else {
            return false;
        };

        let components = NSDateComponents::new();
        components.setHour(isize::from(hour.min(23)));
        components.setMinute(isize::from(minute.min(59)));

        // SAFETY: documented UserNotifications classes, properties, and factory methods.
        unsafe {
            let content: Retained<AnyObject> = msg_send![content_cls, new];
            let () = msg_send![&*content, setTitle: &*NSString::from_str(title)];
            let () = msg_send![&*content, setBody: &*NSString::from_str(body)];
            let sound: Retained<AnyObject> = msg_send![sound_cls, defaultSound];
            let () = msg_send![&*content, setSound: &*sound];

            let trigger: Retained<AnyObject> = msg_send![
                trigger_cls,
                triggerWithDateMatchingComponents: &*components,
                repeats: Bool::YES
            ];
            let request: Retained<AnyObject> = msg_send![
                request_cls,
                requestWithIdentifier: &*NSString::from_str(id),
                content: &*content,
                trigger: &*trigger
            ];
            let done = RcBlock::new(move |error: *mut NSError| {
                // SAFETY: framework passes either a valid NSError or nil.
                if let Some(error) = error.as_ref() {
                    log::warn!(
                        "notifications: daily reminder schedule failed: {}",
                        error.localizedDescription()
                    );
                }
            });
            let () = msg_send![
                &*center,
                addNotificationRequest: &*request,
                withCompletionHandler: &*done
            ];
        }
        true
    }

    pub fn cancel(id: &str) {
        let Some(center) = center() else {
            return;
        };
        let ids = NSArray::from_retained_slice(&[NSString::from_str(id)]);
        // SAFETY: documented methods taking an array of identifier strings.
        unsafe {
            let () = msg_send![&*center, removePendingNotificationRequestsWithIdentifiers: &*ids];
            let () = msg_send![&*center, removeDeliveredNotificationsWithIdentifiers: &*ids];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unbundled_test_process_falls_back_safely_without_crashing() {
        assert!(!schedule_daily_reminder(
            "goal-reminder",
            "Goal reminder",
            "Time to read",
            8,
            30
        ));
        cancel_daily_reminder("goal-reminder");
    }
}
