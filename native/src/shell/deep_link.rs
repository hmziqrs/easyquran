//! Deep links: [`route_from_deep_link`](crate::shell::routes) parses the
//! path. NSH 0.4.0 had no deep-link delivery of its own; 0.5.0's URL
//! schemes are not adopted yet (a follow-up), so the entries are a
//! desktop launch argument (set before the engine runs) and this seam for
//! an embedding host, which calls [`open`] from its `host.on` wiring.

use std::sync::OnceLock;

use crate::shell::{
    nav,
    routes::{Route, route_from_deep_link},
};

/// The scheme the app answers to.
pub const SCHEME: &str = "nsh-starter://";

/// A deep link from the desktop launch arguments, read once at startup.
static PENDING: OnceLock<String> = OnceLock::new();

/// Stages `url` from the launch arguments (`main`, before the engine ran).
pub fn from_launch(url: impl Into<String>) {
    let _ = PENDING.set(url.into());
}

/// Opens `url` (`nsh-starter://<segment>/<segment>…`): the route it names,
/// or `None` when it is not one of the app's. The seam an embedding host's
/// delivery wires into.
pub fn open(url: &str) -> Option<Route> {
    let path = url.strip_prefix(SCHEME)?;
    let route = route_from_deep_link(&path.split('/').collect::<Vec<_>>())?;
    nav::open(route);
    Some(route)
}

/// Opens the launch argument, once, from the shell's install.
pub fn open_pending() {
    if let Some(url) = PENDING.get() {
        open(url);
    }
}
