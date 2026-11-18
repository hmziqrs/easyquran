//! The pushed pages over Home: the shell's nav stack, a global every
//! entry point (sidebar, tabs, palette, back, deep links) drives, with an
//! undo snapshot taken at each change.

use nsh_core::prelude::*;

use crate::shell::{routes::Route, undo::UndoHistory};

/// The pages pushed over Home, bottom to top.
#[derive(Default, Clone, PartialEq)]
pub struct Routes(pub Vec<Route>);

/// The stack, a signal of the surface's.
pub fn stack() -> Signal<Routes> {
    global::<Routes>()
}

/// How many pages are pushed.
pub fn depth() -> usize {
    stack().with_untracked(|routes| routes.0.len())
}

/// The page on top, or Home. Tracked: read it in bindings.
pub fn current() -> Route {
    stack()
        .with(|routes| routes.0.last().copied())
        .unwrap_or(Route::Home)
}

/// Snapshots the stack for undo before a change.
fn record() {
    let current = stack().get_untracked().0;
    global::<UndoHistory>().update(|history| history.record(current));
}

/// Opens `route` in the place of whatever was pushed (a tab, a sidebar
/// row, a palette destination, a deep link). A detail brings its parent.
pub fn open(route: Route) {
    record();
    let pages = match (route.parent(), route) {
        (Some(parent), _) => vec![parent, route],
        (_, Route::Home) => Vec::new(),
        (_, _) => vec![route],
    };
    stack().set(Routes(pages));
}

/// Pushes `route` over what is open; one already in the stack (a double
/// tap) is not pushed twice.
pub fn push(route: Route) {
    record();
    stack().update(|routes| {
        if !routes.0.contains(&route) {
            routes.0.push(route);
        }
    });
}

/// Pops the top page (the shell's back handler), recorded for undo.
pub fn pop() {
    record();
    stack().update(|routes| {
        routes.0.pop();
    });
}

/// The user popped natively (system back, swipe): pages from `depth` up
/// go, recorded for undo.
pub fn truncate(depth: usize) {
    let gone = stack().with_untracked(|routes| routes.0.len() > depth);
    if gone {
        record();
    }
    stack().update(|routes| routes.0.truncate(depth));
}
