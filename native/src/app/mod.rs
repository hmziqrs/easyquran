//! App-level composition owned by the shell area: the window assembly
//! around the shell. Providers for the app's stores land here as the
//! foundation area's exports do; the shell keeps its own state in
//! globals, as NSH's own gallery example does.

use nsh_core::prelude::*;

use crate::shell;

/// The surface's assembly: the shell-wide wiring under the root's owner,
/// the shell inside it.
#[component]
pub fn Root() -> impl IntoElement {
    shell::install();
    view().flex_1().child(shell::shell())
}
