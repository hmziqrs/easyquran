//! Callbacks for a field's or a form's events. A listener sees the value after
//! the event; `on_change_debounce` reports only the last edit of a burst.

use std::rc::Rc;
use std::time::Duration;

/// A listener callback over `V`.
pub(crate) type Callback<V> = Rc<dyn Fn(&V)>;

/// Callbacks for one event each. A second call to the same method replaces the first.
pub struct Listeners<V> {
    pub(crate) on_change: Option<Callback<V>>,
    pub(crate) on_blur: Option<Callback<V>>,
    pub(crate) on_mount: Option<Callback<V>>,
    pub(crate) on_submit: Option<Callback<V>>,
    pub(crate) on_change_debounce: Duration,
}

/// Listeners of a field. The callback gets the field's value.
pub type FieldListeners<T> = Listeners<T>;

/// Listeners of a form. The callback gets the form's values.
pub type FormListeners<H> = Listeners<H>;

impl<V> Default for Listeners<V> {
    fn default() -> Self {
        Self {
            on_change: None,
            on_blur: None,
            on_mount: None,
            on_submit: None,
            on_change_debounce: Duration::ZERO,
        }
    }
}

impl<V> Listeners<V> {
    /// No listeners.
    pub fn new() -> Self {
        Self::default()
    }

    /// Runs after each edit.
    pub fn on_change(mut self, listener: impl Fn(&V) + 'static) -> Self {
        self.on_change = Some(Rc::new(listener));
        self
    }

    /// Runs on blur.
    pub fn on_blur(mut self, listener: impl Fn(&V) + 'static) -> Self {
        self.on_blur = Some(Rc::new(listener));
        self
    }

    /// Runs once when the field or form mounts.
    pub fn on_mount(mut self, listener: impl Fn(&V) + 'static) -> Self {
        self.on_mount = Some(Rc::new(listener));
        self
    }

    /// Runs on submit, after validation passed and before the save.
    pub fn on_submit(mut self, listener: impl Fn(&V) + 'static) -> Self {
        self.on_submit = Some(Rc::new(listener));
        self
    }

    /// Delays the change listener until no edit has come for `delay`. Only the last edit reports.
    pub fn on_change_debounce(mut self, delay: Duration) -> Self {
        self.on_change_debounce = delay;
        self
    }
}
