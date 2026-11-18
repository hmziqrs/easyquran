//! App (PARITY J5, K4; web `AppSection.svelte`, `update.svelte.ts`,
//! `UpdateToast.svelte`): installed and latest version, the update status,
//! "Check for updates", and the update-available toast.
//!
//! Updates come from `nsh-updater`: the packaged app carries its updater
//! config (`nsh-updater.json`, written by `nsh package` from `nsh.json`'s
//! `"updater"`); the manifest is fetched over the app's transport and
//! `Config::check` decides. Without that config (a development build, or a
//! package made with no `"updater"`) the status stays unknown. Applying
//! opens the release asset; installing in place is not wired.

use eq_core::{
    i18n::keys,
    net::{Request, Transport},
};
use nsh_base::theme;
use nsh_core::{element::AnyElement, prelude::*};

use super::section_frame;
use crate::{
    features::downloads::rows::{caption, label_value, list_card, row_frame, row_label},
    services::{files, transport::NshTransport},
    shell::routes::SettingsSection,
    stores::locale::{t, tr},
    ui::{
        button::{ButtonSize, ButtonVariant, button_with, icon_button},
        text::{TextRole, role},
        tokens,
    },
};

#[derive(Clone, Default)]
struct Update {
    checking: bool,
    /// The packaged version, once the updater config is read.
    packaged: Option<String>,
    /// The newest version the manifest offers (this one when up to date).
    latest: Option<String>,
    /// A newer build for this machine: `(version, url)`.
    available: Option<(String, String)>,
    /// The version the toast was dismissed for.
    dismissed: Option<String>,
}

fn state() -> Signal<Update> {
    global::<Update>()
}

/// The running version (tracked).
pub fn installed_version() -> impl Fn() -> SharedString + Copy + 'static {
    || {
        state()
            .with(|u| u.packaged.clone())
            .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_owned())
            .into()
    }
}

/// The latest known version, "—" before a check answered (tracked).
pub fn latest_version() -> SharedString {
    state()
        .with(|u| u.latest.clone())
        .unwrap_or_else(|| "—".to_owned())
        .into()
}

async fn fetch(url: String) -> Option<Vec<u8>> {
    match NshTransport.send(Request::get(url)).await {
        Ok(res) if (200..300).contains(&res.status) => Some(res.body),
        Ok(res) => {
            log::info!("update check: HTTP {}", res.status);
            None
        }
        Err(e) => {
            log::info!("update check: {e}");
            None
        }
    }
}

/// `update.checkNow()`.
pub fn check_now() {
    if state().with_untracked(|u| u.checking) {
        return;
    }
    state().update(|u| u.checking = true);
    spawn(async move {
        let outcome = run_check().await;
        state().update(|u| {
            u.checking = false;
            if let Some((packaged, latest, available)) = outcome {
                u.packaged = Some(packaged);
                u.latest = Some(latest);
                u.available = available;
            }
        });
    });
}

type CheckOutcome = (String, String, Option<(String, String)>);

async fn run_check() -> Option<CheckOutcome> {
    let raw = match files::read_asset(nsh_updater::CONFIG_ASSET).await {
        Ok(raw) => raw,
        Err(e) => {
            log::info!("update check: no updater config ({e})");
            return None;
        }
    };
    let config = match nsh_updater::Config::parse(&raw) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("update check: {e}");
            return None;
        }
    };
    let manifest = fetch(config.manifest_url()).await?;
    let signature = match config.signature_url() {
        Some(url) => Some(fetch(url).await?),
        None => None,
    };
    match config.check(&manifest, signature.as_deref(), &nsh_updater::platform()) {
        Ok(Some(found)) => Some((
            config.version.clone(),
            found.version.clone(),
            Some((found.version, found.url)),
        )),
        Ok(None) => Some((config.version.clone(), config.version.clone(), None)),
        Err(e) => {
            log::warn!("update check: {e}");
            None
        }
    }
}

/// `update.apply()`: opens the new build's download.
fn apply() {
    let Some((_, url)) = state().with_untracked(|u| u.available.clone()) else {
        return;
    };
    spawn(async move {
        if !open_url(url).await {
            log::warn!("update: could not open the download");
        }
    });
}

fn status_text() -> SharedString {
    let u = state().get();
    if u.checking {
        return tr(keys::SETTINGS_APP_STATUS_CHECKING).into();
    }
    if u.available.is_some() {
        return tr(keys::SETTINGS_APP_STATUS_UPDATE_READY).into();
    }
    if u.latest.is_some() && u.latest == u.packaged {
        return tr(keys::SETTINGS_APP_STATUS_UP_TO_DATE).into();
    }
    tr(keys::SETTINGS_APP_STATUS_UNKNOWN).into()
}

pub fn section() -> AnyElement {
    let checking = move || state().with(|u| u.checking);
    let available = move || state().with(|u| u.available.is_some());
    let status = row_frame()
        .flex_row()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap_4()
        .child(
            role(status_text, TextRole::Body)
                .font_weight(FontWeight::MEDIUM)
                .text_color(move || {
                    if state().with(|u| u.available.is_some()) {
                        return theme().primary;
                    }
                    theme().foreground
                }),
        )
        .child(when(available, || {
            button_with(
                t(keys::SETTINGS_APP_RELOAD_UPDATE),
                ButtonVariant::Primary,
                ButtonSize::Small,
                false,
                None,
                apply,
            )
            .into_element()
        }));
    let check = row_frame()
        .flex_row()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap_4()
        .child(row_label(t(keys::SETTINGS_APP_CHECK_UPDATES)))
        .child(button_with(
            t(keys::SETTINGS_APP_CHECK_UPDATES),
            ButtonVariant::Outline,
            ButtonSize::Small,
            checking,
            None,
            check_now,
        ));
    section_frame(
        SettingsSection::App,
        keys::SETTINGS_APP_INTRO,
        list_card(vec![
            label_value(t(keys::SETTINGS_APP_INSTALLED), installed_version()),
            label_value(t(keys::SETTINGS_APP_LATEST), latest_version),
            status.into_element(),
            check.into_element(),
        ]),
    )
}

/// The update-available toast (web `UpdateToast.svelte`): shown while a
/// newer build is available and not dismissed for that version. Checks
/// once when mounted, on the desktops. The shell mounts it once, over the
/// pages, at the top of the window.
pub fn update_toast() -> impl IntoElement {
    if is_desktop() {
        check_now();
    }
    let visible = move || {
        state().with(|u| {
            u.available
                .as_ref()
                .is_some_and(|(v, _)| u.dismissed.as_ref() != Some(v))
        })
    };
    view()
        .absolute()
        .left_0()
        .right_0()
        .top_0()
        .items_center()
        .pt_4()
        .px_3()
        .pointer_events(nsh_core::PointerEvents::BoxNone)
        .child(when(visible, || {
            view()
                .w_full()
                .max_w(px(380.0))
                .flex_row()
                .items_start()
                .gap_3()
                .p(px(14.0))
                .rounded(tokens::radius_xl)
                .border_1()
                .border_color(move || theme().border)
                .bg(tokens::surface)
                .shadow_md()
                .child(
                    view()
                        .size(px(32.0))
                        .flex_shrink_0()
                        .items_center()
                        .justify_center()
                        .rounded(tokens::radius_sm)
                        .bg(tokens::primary_soft)
                        .child(text("↑").text_color(move || theme().primary)),
                )
                .child(
                    view()
                        .flex_1()
                        .min_w_0()
                        .flex_col()
                        .gap_1p5()
                        .child(row_label(t(keys::READER_NEW_VERSION_READY)))
                        .child(caption(t(keys::READER_RELOAD_UPDATE)))
                        .child(view().flex_row().child(button_with(
                            t(keys::READER_RELOAD_OPEN_TABS),
                            ButtonVariant::Primary,
                            ButtonSize::Small,
                            false,
                            None,
                            apply,
                        ))),
                )
                .child(icon_button("✕", tr(keys::READER_DISMISS_UPDATE), || {
                    state().update(|u| u.dismissed = u.available.as_ref().map(|(v, _)| v.clone()));
                }))
                .into_element()
        }))
}
