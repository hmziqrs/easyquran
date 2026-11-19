//! Devices and sessions (PARITY I7; devices.md D4): each registered
//! device with rename and remote sign-out, then every session no listed
//! device owns, with revoke. Labels come only from fixed tables
//! (`eq_core::api::account`), never from the wire.

use eq_core::{
    api::{
        account::{
            device_name_stamp, device_title, device_type_label, is_handheld,
            normalise_device_name_draft, session_device_label, unbound_sessions,
        },
        types::{DeviceView, SessionInfo},
    },
    i18n::keys,
    sync::clock::ms_from_iso,
};
use nsh_core::{
    element::{AnyElement, ImageSrc},
    prelude::*,
};

use super::{generic, go_sign_in, is_anonymous, section_heading, sign_out};
use crate::{
    app::{auth_store, sync_store},
    features::auth::form::{Msg, error_line},
    stores::locale::{locale, t, t_args, tr},
    ui::{
        button::{ButtonSize, ButtonVariant, button_with},
        field::{field, text_field},
        icons::lucide,
        layout::{BadgeTone, badge, card},
        text::muted,
        toast::notify,
        tokens,
    },
};

/// `device-phone`.
const PHONE_ICON: &str =
    lucide!(r#"<rect width="14" height="20" x="5" y="2" rx="2" ry="2"/><path d="M12 18h.01"/>"#);
/// `device-monitor`.
const MONITOR_ICON: &str = lucide!(
    r#"<rect width="20" height="14" x="2" y="3" rx="2"/><path d="M8 21h8"/><path d="M12 17v4"/>"#
);

/// The section's state, one owner (the section).
#[derive(Clone, Copy)]
struct Devices {
    devices: Signal<Vec<DeviceView>>,
    devices_loading: Signal<bool>,
    devices_error: Signal<bool>,
    sessions: Signal<Vec<SessionInfo>>,
    sessions_loading: Signal<bool>,
    sessions_error: Signal<bool>,
    device_pending: Signal<Option<String>>,
    rename_id: Signal<Option<String>>,
    rename_draft: Signal<String>,
    rename_error: Signal<Option<Msg>>,
    rename_pending: Signal<bool>,
    terminate_pending: Signal<Option<i64>>,
    terminate_error: Signal<bool>,
    logout: Signal<bool>,
}

impl Devices {
    fn new(logout: Signal<bool>) -> Self {
        Self {
            devices: signal(Vec::new()),
            devices_loading: signal(false),
            devices_error: signal(false),
            sessions: signal(Vec::new()),
            sessions_loading: signal(false),
            sessions_error: signal(false),
            device_pending: signal(None),
            rename_id: signal(None),
            rename_draft: signal(String::new()),
            rename_error: signal(None),
            rename_pending: signal(false),
            terminate_pending: signal(None),
            terminate_error: signal(false),
            logout,
        }
    }

    fn load_devices(self) {
        let Some(client) = auth_store().client() else {
            self.devices_error.set(true);
            return;
        };
        self.devices_loading.set(true);
        self.devices_error.set(false);
        spawn(async move {
            let result = client.devices().await;
            let _ = self.devices_loading.try_set(false);
            match result {
                Ok(list) => {
                    let _ = self.devices.try_set(list);
                }
                Err(err) if is_anonymous(&err) => go_sign_in(),
                Err(_) => {
                    let _ = self.devices_error.try_set(true);
                }
            }
        });
    }

    fn load_sessions(self) {
        let Some(client) = auth_store().client() else {
            self.sessions_error.set(true);
            return;
        };
        self.sessions_loading.set(true);
        self.sessions_error.set(false);
        spawn(async move {
            let result = client.sessions_list().await;
            let _ = self.sessions_loading.try_set(false);
            match result {
                Ok(list) => {
                    let _ = self.sessions.try_set(list);
                }
                Err(err) if is_anonymous(&err) => go_sign_in(),
                Err(_) => {
                    let _ = self.sessions_error.try_set(true);
                }
            }
        });
    }

    fn refresh_all(self) {
        self.load_devices();
        self.load_sessions();
    }

    fn terminate(self, id: i64, is_current: bool) {
        if self.terminate_pending.get_untracked().is_some() {
            return;
        }
        let Some(client) = auth_store().client() else {
            return;
        };
        self.terminate_pending.set(Some(id));
        self.terminate_error.set(false);
        spawn(async move {
            let result = client.sessions_terminate(id).await;
            let _ = self.terminate_pending.try_set(None);
            match result {
                Ok(()) if is_current => sign_out(self.logout),
                Ok(()) => self.refresh_all(),
                Err(err) if is_anonymous(&err) => go_sign_in(),
                Err(_) => {
                    let _ = self.terminate_error.try_set(true);
                }
            }
        });
    }

    /// Remote sign-out (devices.md D4): acts at once, with a toast and no
    /// dialog.
    fn sign_out_device(self, device: DeviceView) {
        if self.device_pending.get_untracked().is_some() {
            return;
        }
        let auth = auth_store();
        let (Some(user), Some(client)) = (auth.user(), auth.client()) else {
            return;
        };
        self.device_pending.set(Some(device.id.clone()));
        spawn(async move {
            let result = client.device_delete(user.id, &device.id).await;
            let _ = self.device_pending.try_set(None);
            match result {
                Ok(_) => {
                    let name = title_of(&device);
                    notify(crate::features::bookmarks::fmt(
                        keys::ACCOUNT_DEVICE_SIGNED_OUT,
                        &[("name", name.into())],
                    ));
                    self.refresh_all();
                }
                Err(err) if is_anonymous(&err) => go_sign_in(),
                Err(_) => notify(tr(keys::ACCOUNT_GENERIC_ERROR)),
            }
        });
    }

    fn start_rename(self, device: &DeviceView) {
        self.rename_id.set(Some(device.id.clone()));
        self.rename_draft
            .set(device.display_name.clone().unwrap_or_default());
        self.rename_error.set(None);
    }

    fn cancel_rename(self) {
        self.rename_id.set(None);
        self.rename_error.set(None);
    }

    fn save_rename(self, device_id: String) {
        if self.rename_pending.get_untracked() {
            return;
        }
        let auth = auth_store();
        let (Some(user), Some(client)) = (auth.user(), auth.client()) else {
            return;
        };
        let Some(name) = normalise_device_name_draft(&self.rename_draft.get_untracked()) else {
            self.rename_error
                .set(Some(Msg::Key(keys::ACCOUNT_DEVICE_NAME_INVALID)));
            return;
        };
        self.rename_pending.set(true);
        self.rename_error.set(None);
        // The stamp comes from this account's sync clock, so a newer name
        // always wins (contracts/sync.md).
        sync_store().run(device_name_stamp, move |stamp| {
            let Ok(Some(stamp)) = stamp else {
                let _ = self.rename_pending.try_set(false);
                let _ = self.rename_error.try_set(Some(generic()));
                return;
            };
            spawn(async move {
                let result = client
                    .device_rename(user.id, &device_id, &name, &stamp)
                    .await;
                let _ = self.rename_pending.try_set(false);
                match result {
                    Ok(_) => {
                        let _ = self.rename_id.try_set(None);
                        self.load_devices();
                    }
                    Err(err) if is_anonymous(&err) => go_sign_in(),
                    Err(_) => {
                        let _ = self.rename_error.try_set(Some(generic()));
                    }
                }
            });
        });
    }
}

fn title_of(device: &DeviceView) -> String {
    device_title(device).unwrap_or_else(|| tr(keys::ACCOUNT_DEVICE_UNKNOWN).to_owned())
}

/// `sessionLastSeenLabel`: "Oct 12, 07:47" in this device's time zone;
/// `None` for a missing or unreadable stamp. A stamp with no zone is read
/// as UTC.
fn last_seen_label(iso: Option<&str>) -> Option<String> {
    const EN: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    const AR: [&str; 12] = [
        "يناير",
        "فبراير",
        "مارس",
        "أبريل",
        "مايو",
        "يونيو",
        "يوليو",
        "أغسطس",
        "سبتمبر",
        "أكتوبر",
        "نوفمبر",
        "ديسمبر",
    ];
    let iso = iso.filter(|s| !s.is_empty())?;
    let ms = ms_from_iso(iso).or_else(|| ms_from_iso(&format!("{iso}Z")))?;
    let at = chrono::DateTime::from_timestamp_millis(ms)?.with_timezone(&chrono::Local);
    use chrono::{Datelike, Timelike};
    let month = usize::try_from(at.month0()).unwrap_or(0).min(11);
    let (day, hour, minute) = (at.day(), at.hour(), at.minute());
    Some(match locale() {
        eq_core::i18n::Locale::Ar => format!("{day} {}، {hour:02}:{minute:02}", AR[month]),
        eq_core::i18n::Locale::En => format!("{} {day}, {hour:02}:{minute:02}", EN[month]),
    })
}

fn last_seen_line(iso: Option<&str>) -> Option<AnyElement> {
    let label = last_seen_label(iso)?;
    Some(
        text(t_args(keys::ACCOUNT_SESSION_LAST_SEEN, move || {
            vec![("date", label.clone().into())]
        }))
        .text_xs()
        .text_color(tokens::muted_text)
        .into_element(),
    )
}

fn row_title(label: String, current: Option<eq_core::i18n::Key>) -> impl IntoElement {
    view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_2()
        .child(
            text(SharedString::from(label))
                .text_sm()
                .font_weight(FontWeight::MEDIUM),
        )
        .child(current.map(|key| badge(t(key), BadgeTone::Primary)))
}

fn row_frame() -> nsh_core::element::View {
    card()
        .flex_row()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap_3()
        .px_4()
        .py_3()
}

fn rename_form(s: Devices, device_id: String) -> AnyElement {
    let save = move || s.save_rename(device_id.clone());
    let save_press = save.clone();
    card()
        .px_4()
        .py_3()
        .child(field(
            t(keys::ACCOUNT_DEVICE_RENAME_LABEL),
            text_field(s.rename_draft, "")
                .max_length(40_u32)
                .auto_correct(false)
                .disabled(move || s.rename_pending.get())
                .on_submit(move |_: &str| save()),
        ))
        .child(error_line(s.rename_error))
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .child(button_with(
                    t(keys::ACCOUNT_DEVICE_RENAME_SAVE),
                    ButtonVariant::Primary,
                    ButtonSize::Small,
                    move || s.rename_pending.get(),
                    None,
                    save_press,
                ))
                .child(button_with(
                    t(keys::ACCOUNT_DEVICE_RENAME_CANCEL),
                    ButtonVariant::Ghost,
                    ButtonSize::Small,
                    false,
                    None,
                    move || s.cancel_rename(),
                )),
        )
        .into_element()
}

fn device_action_label(s: Devices, id: String, current: bool) -> impl Fn() -> SharedString {
    move || {
        if s.device_pending.with(|p| p.as_deref() == Some(id.as_str())) {
            return t(keys::ACCOUNT_SIGNING_OUT)();
        }
        if current {
            return t(keys::ACCOUNT_SIGN_OUT)();
        }
        t(keys::ACCOUNT_DEVICE_SIGN_OUT)()
    }
}

fn device_row(s: Devices, device: DeviceView) -> AnyElement {
    let renaming = device.id.clone();
    let is_renaming = move || {
        s.rename_id
            .with(|r| r.as_deref() == Some(renaming.as_str()))
    };
    let id = device.id.clone();
    when(is_renaming, move || rename_form(s, id.clone()))
        .otherwise(move || device_summary(s, device.clone()))
        .into_element()
}

fn device_summary(s: Devices, device: DeviceView) -> AnyElement {
    let icon = if is_handheld(&device) {
        PHONE_ICON
    } else {
        MONITOR_ICON
    };
    let current = device.is_current;
    let type_label = device
        .display_name
        .as_ref()
        .map(|_| device_type_label(&device).unwrap_or_else(|| tr(keys::ACCOUNT_DEVICE_UNKNOWN)));
    let pending_id = device.id.clone();
    let pending = move || {
        s.device_pending
            .with(|p| p.as_deref() == Some(pending_id.as_str()))
    };
    let for_rename = device.clone();
    let for_sign_out = device.clone();
    row_frame()
        .child(
            view()
                .flex_1()
                .min_w_0()
                .flex_row()
                .items_start()
                .gap_3()
                .child(
                    svg(ImageSrc::svg_markup(icon))
                        .size(px(18.0))
                        .flex_none()
                        .text_color(tokens::muted_text),
                )
                .child(
                    view()
                        .flex_1()
                        .min_w_0()
                        .flex_col()
                        .gap_0p5()
                        .child(row_title(
                            title_of(&device),
                            current.then_some(keys::ACCOUNT_DEVICE_THIS),
                        ))
                        .child(type_label.map(|label| {
                            text(SharedString::from(label))
                                .text_xs()
                                .text_color(tokens::foreground_secondary)
                        }))
                        .child(last_seen_line(Some(device.last_seen_at.as_str()))),
                ),
        )
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_1()
                .child(button_with(
                    t(keys::ACCOUNT_DEVICE_RENAME),
                    ButtonVariant::Ghost,
                    ButtonSize::Small,
                    false,
                    None,
                    move || s.start_rename(&for_rename),
                ))
                .child(button_with(
                    device_action_label(s, device.id.clone(), current),
                    ButtonVariant::Ghost,
                    ButtonSize::Small,
                    pending,
                    None,
                    move || {
                        if current {
                            sign_out(s.logout);
                            return;
                        }
                        s.sign_out_device(for_sign_out.clone());
                    },
                )),
        )
        .into_element()
}

fn session_action_label(s: Devices, id: i64, current: bool) -> impl Fn() -> SharedString {
    move || {
        if s.terminate_pending.get() == Some(id) {
            return t(keys::ACCOUNT_SESSION_ENDING)();
        }
        if current {
            return t(keys::ACCOUNT_SIGN_OUT)();
        }
        t(keys::ACCOUNT_SESSION_REVOKE)()
    }
}

fn session_row(s: Devices, session: SessionInfo) -> AnyElement {
    let label = session_device_label(session.device.as_deref())
        .unwrap_or_else(|| tr(keys::ACCOUNT_DEVICE_UNKNOWN).to_owned());
    let (id, current) = (session.id, session.is_current);
    row_frame()
        .child(
            view()
                .flex_1()
                .min_w_0()
                .flex_col()
                .gap_0p5()
                .child(row_title(
                    label,
                    current.then_some(keys::ACCOUNT_SESSION_THIS),
                ))
                .child(last_seen_line(session.last_seen.as_deref())),
        )
        .child(button_with(
            session_action_label(s, id, current),
            ButtonVariant::Ghost,
            ButtonSize::Small,
            move || s.terminate_pending.get() == Some(id),
            None,
            move || s.terminate(id, current),
        ))
        .into_element()
}

fn retry_line(error: Signal<bool>, on_retry: impl Fn() + Copy + 'static) -> impl IntoElement {
    when(
        move || error.get(),
        move || {
            view()
                .flex_col()
                .gap_2()
                .child(error_line(signal(Some(generic()))))
                .child(view().flex_row().child(button_with(
                    t(keys::ACCOUNT_RETRY),
                    ButtonVariant::Ghost,
                    ButtonSize::Small,
                    false,
                    None,
                    on_retry,
                )))
                .into_element()
        },
    )
}

fn rows(s: Devices) -> AnyElement {
    dynamic(move || {
        let devices = s.devices.get();
        let sessions = s.sessions.with(|list| unbound_sessions(list, &devices));
        if devices.is_empty() && sessions.is_empty() {
            if s.devices_loading.get() || s.sessions_loading.get() {
                return muted(t(keys::ACCOUNT_DEVICES_LOADING)).into_element();
            }
            if s.devices_error.get() || s.sessions_error.get() {
                return view().into_element();
            }
            return muted(t(keys::ACCOUNT_SESSIONS_EMPTY)).into_element();
        }
        view()
            .w_full()
            .flex_col()
            .gap_2()
            .children(devices.into_iter().map(|d| device_row(s, d)))
            .children(sessions.into_iter().map(|x| session_row(s, x)))
            .child(when(
                move || s.terminate_error.get(),
                || error_line(signal(Some(generic()))).into_element(),
            ))
            .into_element()
    })
    .into_element()
}

pub(super) fn devices_section(logout: Signal<bool>) -> AnyElement {
    let s = Devices::new(logout);
    s.refresh_all();
    view()
        .w_full()
        .flex_col()
        .gap_4()
        .child(
            view()
                .flex_col()
                .gap_1()
                .child(section_heading(t(keys::ACCOUNT_DEVICES_HEADING)))
                .child(muted(t(keys::ACCOUNT_DEVICES_DESCRIPTION))),
        )
        .child(retry_line(s.devices_error, move || s.refresh_all()))
        .child(retry_line(s.sessions_error, move || s.load_sessions()))
        .child(rows(s))
        .into_element()
}
