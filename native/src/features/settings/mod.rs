//! Settings shell and sections (PARITY J1-J9, N1). Contract:
//! `pub fn page(section: SettingsSection) -> impl IntoElement`.
//!
//! Mirrors `web/src/routes/(application)/settings/+page.svelte` and
//! `SettingsShell.svelte`: the sections in the web's order (Storage,
//! Appearance, Reading, App, Privacy, Your data, Account), one shown at a
//! time. Desktop: the section list beside the pane. Phones: the web's pill
//! row above the section (a pushed section page cannot work here: every
//! section shares the settings route's stack slot, and `Settings(Storage)`
//! is the tab itself).
//!
//! Also public: [`update_toast`] (the update-available toast, PARITY K4,
//! for the shell to mount once), [`installed_version`] / [`latest_version`]
//! (the downloads screen's App rows) and [`goal_reminders`] (stub).

mod account;
mod app;
mod appearance;
mod data;
mod privacy;
mod reading;
mod storage;

use eq_core::i18n::{Key, keys};
use nsh_core::{element::AnyElement, prelude::*};

pub use app::{installed_version, latest_version, update_toast};
pub(crate) use data::export as export_data;
pub use privacy::goal_reminders;

use crate::{
    features::downloads::rows::heading,
    shell::routes::SettingsSection,
    stores::locale::{locale, t, tr},
    ui::{
        button::choice_button,
        layout::{page_body, page_insets},
        nav::nav_row,
        text::{TextRole, page_heading, role},
        tokens,
    },
};

fn section_key(section: SettingsSection) -> Key {
    match section {
        SettingsSection::Storage => keys::SETTINGS_SECTION_STORAGE,
        SettingsSection::Appearance => keys::SETTINGS_SECTION_APPEARANCE,
        SettingsSection::Reading => keys::SETTINGS_SECTION_READING,
        SettingsSection::App => keys::SETTINGS_SECTION_APP,
        SettingsSection::Privacy => keys::SETTINGS_SECTION_PRIVACY,
        SettingsSection::Data => keys::SETTINGS_SECTION_DATA,
        SettingsSection::Account => keys::SETTINGS_SECTION_ACCOUNT,
    }
}

fn nav_test_id(section: SettingsSection) -> &'static str {
    match section {
        SettingsSection::Storage => "settings-nav-storage",
        SettingsSection::Appearance => "settings-nav-appearance",
        SettingsSection::Reading => "settings-nav-reading",
        SettingsSection::App => "settings-nav-app",
        SettingsSection::Privacy => "settings-nav-privacy",
        SettingsSection::Data => "settings-nav-data",
        SettingsSection::Account => "settings-nav-account",
    }
}

fn section_body(section: SettingsSection) -> AnyElement {
    match section {
        SettingsSection::Storage => storage::section(),
        SettingsSection::Appearance => appearance::section(),
        SettingsSection::Reading => reading::section(),
        SettingsSection::App => app::section(),
        SettingsSection::Privacy => privacy::section(),
        SettingsSection::Data => data::section(),
        SettingsSection::Account => account::section(),
    }
}

/// A section's frame: its heading, its intro line, then its content.
pub(crate) fn section_frame(
    section: SettingsSection,
    intro: Key,
    content: impl IntoElement,
) -> AnyElement {
    view()
        .w_full()
        .flex_col()
        .gap_4()
        .child(
            view()
                .flex_col()
                .gap_1()
                .child(heading(t(section_key(section))))
                .child(
                    role(t(intro), TextRole::Body)
                        .text_size(px(14.5))
                        .text_color(tokens::foreground_secondary),
                ),
        )
        .child(content)
        .into_element()
}

/// One pill of a choice row (the web's toggle pill).
pub(crate) fn pill(
    label: impl IntoReactive<SharedString>,
    name: &'static str,
    selected: impl IntoReactive<bool>,
    on_press: impl Fn() + 'static,
) -> AnyElement {
    choice_button(label, name, selected, false, on_press).into_element()
}

fn desktop_list(active: Signal<SettingsSection>) -> AnyElement {
    dynamic(move || {
        locale();
        view()
            .w(px(220.0))
            .flex_shrink_0()
            .flex_col()
            .gap_0p5()
            .label(tr(keys::SETTINGS_SECTIONS_LABEL))
            .children(SettingsSection::ALL.into_iter().map(|s| {
                nav_row(
                    tr(section_key(s)),
                    None,
                    move || active.get() == s,
                    nav_test_id(s),
                    move || active.set(s),
                )
            }))
            .into_element()
    })
    .into_element()
}

fn phone_pills(active: Signal<SettingsSection>) -> AnyElement {
    scroll()
        .horizontal()
        .w_full()
        .child(
            view()
                .flex_row()
                .gap_1p5()
                .label(tr(keys::SETTINGS_SECTIONS_LABEL))
                .children(SettingsSection::ALL.into_iter().map(|s| {
                    pill(
                        t(section_key(s)),
                        nav_test_id(s),
                        move || active.get() == s,
                        move || active.set(s),
                    )
                })),
        )
        .into_element()
}

fn pane(active: Signal<SettingsSection>) -> AnyElement {
    dynamic(move || section_body(active.get())).into_element()
}

pub fn page(section: SettingsSection) -> impl IntoElement {
    crate::features::downloads::dev_capture();
    let active = signal(section);
    if is_desktop() {
        return view()
            .flex_1()
            .flex_row()
            .child(
                view()
                    .px_3()
                    .py_4()
                    .flex_col()
                    .gap_3()
                    .child(page_heading(t(keys::SETTINGS_TITLE)))
                    .child(desktop_list(active)),
            )
            .child(
                scroll()
                    .flex_1()
                    .content_inset(page_insets(0.0))
                    .child(page_body().max_w(px(780.0)).child(pane(active))),
            )
            .into_element();
    }
    scroll()
        .flex_1()
        .content_inset(page_insets(0.0))
        .child(
            page_body()
                .child(page_heading(t(keys::SETTINGS_TITLE)))
                .child(phone_pills(active))
                .child(pane(active)),
        )
        .into_element()
}
