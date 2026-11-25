//! The goal reminder on iOS and macOS: a repeating calendar trigger with
//! `UNUserNotificationCenter`. NSH owns the center's delegate; nothing here
//! touches it.

use std::{ffi::CStr, sync::OnceLock};

use block2::RcBlock;
use objc2::{
    msg_send,
    rc::Retained,
    runtime::{AnyClass, AnyObject, Bool},
};
use objc2_foundation::{NSArray, NSBundle, NSDateComponents, NSError, NSString};

use super::DailyReminder;

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
pub(super) fn center() -> Option<Retained<AnyObject>> {
    if !framework() || NSBundle::mainBundle().bundleIdentifier().is_none() {
        return None;
    }
    let class = AnyClass::get(c"UNUserNotificationCenter")?;
    // SAFETY: documented class method, guarded by bundleIdentifier check.
    Some(unsafe { msg_send![class, currentNotificationCenter] })
}

pub(super) fn schedule_daily(reminder: &DailyReminder) -> bool {
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
    let (hour, minute) = reminder.clock();
    components.setHour(hour.into());
    components.setMinute(minute.into());
    if let Some(class) = AnyClass::get(c"NSTimeZone") {
        // SAFETY: documented factory (nil for a zone it does not know) and
        // property.
        unsafe {
            let zone: Option<Retained<AnyObject>> =
                msg_send![class, timeZoneWithName: &*NSString::from_str(&reminder.zone)];
            if let Some(zone) = zone {
                let () = msg_send![&*components, setTimeZone: &*zone];
            }
        }
    }

    // SAFETY: documented UserNotifications classes, properties, and factory methods.
    unsafe {
        let content: Retained<AnyObject> = msg_send![content_cls, new];
        let () = msg_send![&*content, setTitle: &*NSString::from_str(&reminder.title)];
        let () = msg_send![&*content, setBody: &*NSString::from_str(&reminder.body)];
        let sound: Retained<AnyObject> = msg_send![sound_cls, defaultSound];
        let () = msg_send![&*content, setSound: &*sound];

        let trigger: Retained<AnyObject> = msg_send![
            trigger_cls,
            triggerWithDateMatchingComponents: &*components,
            repeats: Bool::YES
        ];
        let request: Retained<AnyObject> = msg_send![
            request_cls,
            requestWithIdentifier: &*NSString::from_str(reminder.id),
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

pub(super) fn cancel(id: &str) {
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
