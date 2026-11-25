//! The goal reminder on Windows: one scheduled toast a day for the next
//! [`DAYS`] days, which the system shows while the app is closed. Windows
//! repeats no scheduled toast, so each start of the app schedules the days
//! ahead again; an app not opened for that long stops reminding.
//!
//! Toasts need an identity of the app's: an MSIX package's, or the
//! AppUserModelID NSH gave the process and registered (NSH does that when the
//! app first asks about notifications). Without one this answers `false`.
//!
//! An unpackaged app's toast opens [`press_link`](super::press_link), a link
//! of the scheme NSH's release runner registers. A package's manifest
//! declares no scheme, so its toast only starts the app.

use std::sync::{
    Mutex, PoisonError,
    atomic::{AtomicU64, Ordering},
};

use windows::{
    Data::Xml::Dom::XmlDocument,
    Globalization::{Calendar, ClockIdentifiers},
    UI::Notifications::{
        NotificationSetting, ScheduledToastNotification, ToastNotificationManager, ToastNotifier,
    },
    Win32::{
        Foundation::{APPMODEL_ERROR_NO_PACKAGE, ERROR_NOT_FOUND, ERROR_SUCCESS},
        Storage::Packaging::Appx::GetCurrentPackageFullName,
        System::{
            Com::CoTaskMemFree,
            Registry::{HKEY_CURRENT_USER, RRF_RT_REG_SZ, RegGetValueW},
        },
        UI::Shell::GetCurrentProcessExplicitAppUserModelID,
    },
    core::{HSTRING, Result, w},
};

use super::{DailyReminder, press_link};

/// How many days ahead are scheduled.
const DAYS: i32 = 30;
const GROUP: &str = "reminders";

/// The last schedule or cancel asked for: an older one still waiting for the
/// system leaves the newer alone.
static LATEST: AtomicU64 = AtomicU64::new(0);
/// One caller changes the schedule at a time.
static SCHEDULE: Mutex<()> = Mutex::new(());

/// Who the system knows the app as.
#[derive(Clone)]
enum Identity {
    Package,
    AppId(HSTRING),
}

impl Identity {
    fn current() -> Option<Self> {
        let mut length = 0;
        // SAFETY: asks for the length only.
        let packaged = unsafe { GetCurrentPackageFullName(&mut length, None) };
        if packaged != APPMODEL_ERROR_NO_PACKAGE {
            return Some(Self::Package);
        }
        // SAFETY: the string is the caller's, freed after it is copied.
        let id = unsafe {
            let raw = GetCurrentProcessExplicitAppUserModelID().ok()?;
            let id = raw.to_hstring();
            CoTaskMemFree(Some(raw.0.cast_const().cast()));
            id
        };
        registered(&id).then_some(Self::AppId(id))
    }

    fn notifier(&self) -> Result<ToastNotifier> {
        match self {
            Self::Package => ToastNotificationManager::CreateToastNotifier(),
            Self::AppId(id) => ToastNotificationManager::CreateToastNotifierWithId(id),
        }
    }
}

/// Whether the id has the registration unpackaged toasts need.
fn registered(id: &HSTRING) -> bool {
    let key = HSTRING::from(format!(
        "Software\\Classes\\AppUserModelId\\{}",
        id.to_string_lossy()
    ));
    // SAFETY: valid strings; no value is read.
    let found = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            &key,
            w!("DisplayName"),
            RRF_RT_REG_SZ,
            None,
            None,
            None,
        )
    };
    found == ERROR_SUCCESS
}

/// `text` as XML content or an attribute's, without what XML cannot hold.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c if c < ' ' => {}
            c => out.push(c),
        }
    }
    out
}

fn toast_xml(reminder: &DailyReminder, identity: &Identity) -> String {
    let press = match identity {
        Identity::Package => String::new(),
        Identity::AppId(_) => format!(
            " activationType=\"protocol\" launch=\"{}\"",
            escape(&press_link())
        ),
    };
    format!(
        "<toast{press}><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text></binding></visual></toast>",
        escape(&reminder.title),
        escape(&reminder.body)
    )
}

/// Today at the reminder's minute, in its zone (the device's when Windows
/// does not know it).
fn today(reminder: &DailyReminder) -> Result<Calendar> {
    let at = Calendar::new()?;
    at.ChangeClock(&ClockIdentifiers::TwentyFourHour()?)?;
    if at.ChangeTimeZone(&HSTRING::from(&reminder.zone)).is_err() {
        log::info!("notifications: a reminder zone Windows does not know");
    }
    at.SetToNow()?;
    let (hour, minute) = reminder.clock();
    at.SetHour(hour.into())?;
    at.SetMinute(minute.into())?;
    at.SetSecond(0)?;
    at.SetNanosecond(0)?;
    Ok(at)
}

fn remove_all(notifier: &ToastNotifier, id: &str) -> Result<()> {
    let id = HSTRING::from(id);
    for toast in notifier.GetScheduledToastNotifications()? {
        if toast.Id()? == id {
            notifier.RemoveFromSchedule(&toast)?;
        }
    }
    Ok(())
}

fn schedule(notifier: &ToastNotifier, reminder: &DailyReminder, identity: &Identity) -> Result<()> {
    remove_all(notifier, reminder.id)?;
    let document = XmlDocument::new()?;
    document.LoadXml(&HSTRING::from(toast_xml(reminder, identity)))?;
    let now = Calendar::new()?;
    now.SetToNow()?;
    let first = today(reminder)?;
    let first_day = if first.Compare(&now)? > 0 { 0 } else { 1 };
    for day in first_day..first_day + DAYS {
        // From today each time: a day whose clock skips the minute moves
        // only itself.
        let at = first.Clone()?;
        at.AddDays(day)?;
        let toast = ScheduledToastNotification::CreateScheduledToastNotification(
            &document,
            at.GetDateTime()?,
        )?;
        toast.SetId(&HSTRING::from(reminder.id))?;
        toast.SetTag(&HSTRING::from(reminder.id))?;
        toast.SetGroup(&HSTRING::from(GROUP))?;
        notifier.AddToSchedule(&toast)?;
    }
    Ok(())
}

/// Runs `change` off the engine's thread (each toast is a call to the
/// system), unless a later change was asked for before its turn.
fn later(change: impl FnOnce() -> Result<()> + Send + 'static) {
    let turn = LATEST.fetch_add(1, Ordering::SeqCst) + 1;
    std::thread::spawn(move || {
        let _one = SCHEDULE.lock().unwrap_or_else(PoisonError::into_inner);
        if LATEST.load(Ordering::SeqCst) != turn {
            return;
        }
        if let Err(error) = change() {
            log::warn!("notifications: daily reminder schedule failed: {error}");
        }
    });
}

pub(super) fn schedule_daily(reminder: &DailyReminder) -> bool {
    let Some(identity) = Identity::current() else {
        return false;
    };
    let Ok(notifier) = identity.notifier() else {
        return false;
    };
    let on = match notifier.Setting() {
        Ok(setting) => setting == NotificationSetting::Enabled,
        // No toast of the app's has shown yet: nothing is turned off.
        Err(error) => error.code() == ERROR_NOT_FOUND.to_hresult(),
    };
    if !on {
        return false;
    }
    let reminder = reminder.clone();
    later(move || schedule(&identity.notifier()?, &reminder, &identity));
    true
}

pub(super) fn cancel(id: &str) {
    let Some(identity) = Identity::current() else {
        return;
    };
    let id = id.to_owned();
    later(move || {
        remove_all(&identity.notifier()?, &id)?;
        let history = ToastNotificationManager::History()?;
        let (tag, group) = (HSTRING::from(&id), HSTRING::from(GROUP));
        match &identity {
            Identity::Package => history.RemoveGroupedTag(&tag, &group),
            Identity::AppId(app) => history.RemoveGroupedTagWithId(&tag, &group, app),
        }
    });
}
