//! Deep links (docs/plans/native-shared.md N4, contracts/routes.md):
//! `easyquran://…` links (every platform, through NSH core's URL schemes,
//! `nsh.json` `"schemes"`) and `https://easyquran.fyi/…` Universal Links
//! (iOS, macOS) and App Links (Android), delivered by `nsh-deep-links`.
//!
//! Every link is parsed by `eq_core::routes::parse_url` and opened through
//! [`route_from_url`] (which maps `parse_url`'s link with
//! `route_from_link`, plus the native-only paths). A link of the app whose
//! path names no screen, or a reader / zikr target that does not exist,
//! opens the not-found page (web `+error.svelte` 404).
//! A desktop launch argument (`easyquran easyquran://surah/al-fatiha`) is
//! read by `main` too, for `cargo run` and hosts that pass the URL on the
//! command line; the same URL coming again through the platform is skipped
//! once.

use std::sync::OnceLock;

use eq_core::routes::HOSTS;
use nsh_core::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use nsh_deep_links::{DeepLinks, ErrorKind, RouteTable};

use crate::shell::{
    nav,
    routes::{Route, route_from_url},
};

/// The scheme the app answers to.
pub const SCHEME: &str = "easyquran://";

/// A deep link from the desktop launch arguments, read once at startup.
static PENDING: OnceLock<String> = OnceLock::new();

/// Stages `url` from the launch arguments (`main`, before the engine ran).
pub fn from_launch(url: impl Into<String>) {
    let _ = PENDING.set(url.into());
}

/// Opens `url`: the route it names; an app link whose path names no page
/// (or a target that does not exist) opens the not-found page, as the web
/// answers 404. `None` when the link is not the app's.
pub fn open(url: &str) -> Option<Route> {
    let route = route_from_url(url)?;
    nav::open(route.clone());
    Some(route)
}

/// Opens the launch argument, once, then listens for the platform's links
/// for the rest of the run (called from the shell's install).
pub fn open_pending() {
    if let Some(url) = PENDING.get() {
        open(url);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        spawn(listen());
    }
}

/// The library's table: the app's prefixes (its scheme, and the hosts as
/// the verified-link allowlist), with one catch-all screen, so every link
/// of the app reaches [`open`], which routes by `eq_core`.
#[cfg(not(target_arch = "wasm32"))]
fn table() -> Option<RouteTable> {
    let mut prefixes: Vec<String> = HOSTS.iter().map(|h| format!("https://{h}")).collect();
    prefixes.push(SCHEME.to_owned());
    let config = serde_json::json!({
        "prefixes": prefixes,
        "screens": { "Root": "", "Any": "*" },
    });
    match RouteTable::from_config(&config) {
        Ok(table) => Some(table),
        Err(e) => {
            log::warn!("deep links: route table refused ({})", e.code());
            None
        }
    }
}

/// Whether delivery ends after `kind` (a cancel or a platform failure),
/// not just the one link.
#[cfg(not(target_arch = "wasm32"))]
fn ends_delivery(kind: ErrorKind) -> bool {
    !matches!(
        kind,
        ErrorKind::InvalidUrl
            | ErrorKind::UrlTooLarge
            | ErrorKind::DisallowedScheme
            | ErrorKind::HostNotAllowed
            | ErrorKind::NoRoute
            | ErrorKind::InvalidParameter
            | ErrorKind::UnsupportedPlatform
    )
}

#[cfg(not(target_arch = "wasm32"))]
async fn listen() {
    let Some(table) = table() else {
        return;
    };
    let links = DeepLinks::platform(table);
    let mut skip = PENDING.get().cloned();
    match links.initial().await {
        Ok(Some(link)) => {
            if skip.as_deref() == Some(link.url.as_str()) {
                skip = None;
            } else {
                open(&link.url);
            }
        }
        Ok(None) => {}
        Err(e) => log::info!("deep links: launch link refused ({})", e.code()),
    }
    loop {
        match links.next().await {
            Ok(link) => {
                if skip.as_deref() == Some(link.url.as_str()) {
                    skip = None;
                    continue;
                }
                open(&link.url);
            }
            Err(e) if ends_delivery(e.kind()) => {
                log::info!("deep links: delivery ended ({})", e.code());
                return;
            }
            Err(e) => log::info!("deep links: link refused ({})", e.code()),
        }
    }
}
