//! The goal reminder on Android: an inexact daily alarm (`AlarmManager`)
//! whose receiver shows it, the app's process running or not. The receiver is
//! the app's own class (`native/android/reminders`, packaged and declared by
//! `nsh.json` `android`), called over JNI; it keeps the schedule and sets its
//! alarm again after a restart, an update and a clock or zone change.
//!
//! A press opens [`press_link`](super::press_link), the app's own link.

use jni::{
    JNIEnv, JavaVM,
    errors::Result,
    objects::{JClass, JObject, JValue},
};

use super::{DailyReminder, press_link};

const RECEIVER: &str = "fyi.easyquran.reminders.ReminderReceiver";

/// Runs `call` with the application's `Context` and the receiver's class.
/// The class is the app's own, so its loader finds it, not `FindClass`,
/// which on this thread knows the system's only.
fn with_receiver<T>(
    call: impl FnOnce(&mut JNIEnv<'_>, &JObject<'_>, &JClass<'_>) -> Result<T>,
) -> Result<T> {
    let android = ndk_context::android_context();
    // SAFETY: NSH sets the process's VM and application `Context` before the
    // app's code runs; both last as long as the process.
    let vm = unsafe { JavaVM::from_raw(android.vm().cast()) }?;
    let mut env = vm.attach_current_thread()?;
    // SAFETY: as above; a global reference, which `JObject` does not delete.
    let context = unsafe { JObject::from_raw(android.context().cast()) };
    let answer = env.with_local_frame(16, |env| -> Result<T> {
        let loader = env
            .call_method(&context, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])?
            .l()?;
        let name = env.new_string(RECEIVER)?;
        let class = env
            .call_method(
                &loader,
                "loadClass",
                "(Ljava/lang/String;)Ljava/lang/Class;",
                &[JValue::Object(&name)],
            )?
            .l()?;
        call(env, &context, &JClass::from(class))
    });
    if answer.is_err() {
        // A pending exception would end the next call on this thread.
        let _ = env.exception_clear();
    }
    answer
}

fn minutes(quiet: Option<(u16, u16)>) -> (i32, i32) {
    quiet.map_or((-1, -1), |(start, end)| (start.into(), end.into()))
}

pub(super) fn schedule_daily(reminder: &DailyReminder) -> bool {
    let (quiet_start, quiet_end) = minutes(reminder.quiet);
    let scheduled = with_receiver(|env, context, class| {
        let id = env.new_string(reminder.id)?;
        let title = env.new_string(&reminder.title)?;
        let body = env.new_string(&reminder.body)?;
        let link = env.new_string(press_link())?;
        let zone = env.new_string(&reminder.zone)?;
        env.call_static_method(
            class,
            "schedule",
            "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;\
             Ljava/lang/String;Ljava/lang/String;ILjava/lang/String;II)Z",
            &[
                JValue::Object(context),
                JValue::Object(&id),
                JValue::Object(&title),
                JValue::Object(&body),
                JValue::Object(&link),
                JValue::Int(reminder.minute.min(1439).into()),
                JValue::Object(&zone),
                JValue::Int(quiet_start),
                JValue::Int(quiet_end),
            ],
        )?
        .z()
    });
    scheduled.unwrap_or_else(|error| {
        log::info!("notifications: no reminder receiver: {error}");
        false
    })
}

pub(super) fn cancel(id: &str) {
    let cancelled = with_receiver(|env, context, class| {
        let id = env.new_string(id)?;
        env.call_static_method(
            class,
            "cancel",
            "(Landroid/content/Context;Ljava/lang/String;)V",
            &[JValue::Object(context), JValue::Object(&id)],
        )?
        .v()
    });
    if let Err(error) = cancelled {
        log::info!("notifications: no reminder receiver: {error}");
    }
}
