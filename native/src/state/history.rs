//! Undo and redo history: the plain record the shell's commands write to,
//! one per document or store, kept by its owner in a `stored(..)` handle.

/// An undo and redo record of states `T`, oldest first. Not reactive: the
/// owner reads and writes it in handlers and mirrors what UI shows
/// (`can_undo`) into its own signals.
#[derive(Debug)]
pub struct History<T> {
    undo: Vec<T>,
    redo: Vec<T>,
    limit: usize,
}

impl<T: Clone> History<T> {
    /// A history keeping at most `limit` past states (a `limit` of 0 keeps
    /// none: every `push` is a plain overwrite).
    pub fn new(limit: usize) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            limit,
        }
    }

    /// Records `state` as the newest past state; redo forgets what a new
    /// edit replaced.
    pub fn push(&mut self, state: T) {
        self.undo.push(state);
        if self.undo.len() > self.limit {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Steps back, giving the state to restore (`None` at the oldest).
    pub fn undo(&mut self, current: T) -> Option<T> {
        let state = self.undo.pop()?;
        self.redo.push(current);
        Some(state)
    }

    /// Steps forward, giving the state to restore (`None` at the newest).
    pub fn redo(&mut self, current: T) -> Option<T> {
        let state = self.redo.pop()?;
        self.undo.push(current);
        Some(state)
    }

    /// Whether [`History::undo`] has a state to give.
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Whether [`History::redo`] has a state to give.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Break seen: `History::undo` peeked (`last()`) instead of popping — failed at the second `undo` assertion.
    #[test]
    fn undo_redo_and_limit_order_states() {
        let mut history = History::new(2);
        assert!(!history.can_undo());
        history.push(1);
        history.push(2);
        history.push(3);
        // The limit keeps the newest two.
        assert_eq!(history.undo(4), Some(3));
        assert_eq!(history.undo(3), Some(2));
        assert_eq!(history.undo(2), None);
        assert_eq!(history.redo(2), Some(3));
        assert!(history.can_redo());
        assert_eq!(history.redo(3), Some(4));
        assert!(!history.can_redo());
        // A new edit forgets the redo branch.
        history.push(5);
        assert_eq!(history.redo(5), None);
    }
}
