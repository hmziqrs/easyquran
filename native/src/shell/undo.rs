//! Bounded undo and redo over the shell's own navigation: `nav` snapshots
//! the stack at each change, and undo and redo walk those snapshots.

use nsh_core::prelude::*;

use crate::shell::{nav, routes::Route};

/// How many steps undo keeps; older snapshots go.
const LIMIT: usize = 50;

/// The stack states undo can return to and redo can replay, most recent
/// last.
#[derive(Default, Clone, PartialEq)]
pub struct UndoHistory {
    past: Vec<Vec<Route>>,
    future: Vec<Vec<Route>>,
}

impl UndoHistory {
    /// Records `current` as the state to come back to; the redo tail goes.
    pub fn record(&mut self, current: Vec<Route>) {
        self.past.push(current);
        let over = self.past.len().saturating_sub(LIMIT);
        self.past.drain(0..over);
        self.future.clear();
    }

    /// The state before the last recorded change, if any.
    pub fn undo(&mut self, current: Vec<Route>) -> Option<Vec<Route>> {
        let previous = self.past.pop()?;
        self.future.push(current);
        Some(previous)
    }

    /// The state an undo undid, if any.
    pub fn redo(&mut self, current: Vec<Route>) -> Option<Vec<Route>> {
        let next = self.future.pop()?;
        self.past.push(current);
        Some(next)
    }
}

/// Steps back one recorded navigation.
pub fn undo() {
    let current = nav::stack().get_untracked().0;
    let mut history = global::<UndoHistory>().get_untracked();
    if let Some(previous) = history.undo(current) {
        global::<UndoHistory>().set(history);
        nav::stack().set(nav::Routes(previous));
    }
}

/// Replays the navigation undo undid.
pub fn redo() {
    let current = nav::stack().get_untracked().0;
    let mut history = global::<UndoHistory>().get_untracked();
    if let Some(next) = history.redo(current) {
        global::<UndoHistory>().set(history);
        nav::stack().set(nav::Routes(next));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn steps() -> Vec<Route> {
        vec![Route::Forms]
    }

    #[test]
    fn undo_returns_to_the_recorded_state() {
        let mut history = UndoHistory::default();
        assert_eq!(history.undo(Vec::new()), None);
        history.record(steps());
        // Undo returns the recorded state to restore; the caller sets it.
        assert_eq!(history.undo(Vec::new()), Some(steps()));
        // Redo replays the state undo left: the one `undo` was given.
        assert_eq!(history.redo(steps()), Some(Vec::new()));
    }

    #[test]
    fn the_stack_is_bounded() {
        let mut history = UndoHistory::default();
        for depth in 0..(LIMIT + 10) {
            history.record(vec![Route::DataDetail(depth as u64)]);
        }
        assert_eq!(history.past.len(), LIMIT);
        assert_eq!(history.past[0], vec![Route::DataDetail(10)]);
    }
}
