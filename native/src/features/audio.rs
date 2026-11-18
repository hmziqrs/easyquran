//! Audio dock and in-reader controls (PARITY D3). Not mounted yet: the
//! shell adds the dock above the tab bar / status bar when D2 lands.
//! Contract: `pub fn dock() -> impl IntoElement`.

use nsh_core::prelude::*;

pub fn dock() -> impl IntoElement {
    view()
}
