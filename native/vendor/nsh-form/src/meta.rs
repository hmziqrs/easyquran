//! Per-field metadata: touched, blurred, changed, errors and pending async runs.
//! It is keyed by [`FieldPath`], so an array row's metadata moves with the row.

use std::collections::HashMap;

use gpui_form_core::{FieldPath, PathSegment};
use nsh_core::prelude::*;

use crate::validation::{CAUSE_COUNT, ErrorMap, ValidationError};

/// What a field knows about itself. [`crate::Field::meta`] returns one.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldMeta {
    /// Edited, blurred or submitted at least once since the last reset.
    pub is_touched: bool,
    /// Left by the user (or blurred in code).
    pub is_blurred: bool,
    /// Edited since the last reset. Stays true after the value returns to its default.
    pub was_changed: bool,
    /// The value equals the baseline value.
    pub is_default_value: bool,
    /// An async validator is running or waiting for its debounce.
    pub is_validating: bool,
    /// The errors of the field, keyed by cause. Form-level errors win over the field's own.
    pub error_map: ErrorMap,
}

impl FieldMeta {
    /// Never changed since the last reset.
    pub fn is_pristine(&self) -> bool {
        !self.was_changed
    }

    /// No cause has an error.
    pub fn is_valid(&self) -> bool {
        self.error_map.is_empty()
    }

    /// The distinct error messages.
    pub fn errors(&self) -> Vec<ValidationError> {
        self.error_map.messages()
    }
}

/// The stored metadata of one field. `own` holds the field's own validators'
/// errors, `from_form` the form-level validators' errors.
#[derive(Clone, Debug, Default)]
pub(crate) struct Entry {
    pub touched: bool,
    pub blurred: bool,
    pub dirty: bool,
    pub own: ErrorMap,
    pub from_form: ErrorMap,
    pub pending: [bool; CAUSE_COUNT],
}

impl Entry {
    pub fn effective(&self) -> ErrorMap {
        self.from_form.over(&self.own)
    }

    pub fn is_validating(&self) -> bool {
        self.pending.iter().any(|pending| *pending)
    }
}

/// The signals that mirror one path's state. They are made under the form's owner, so a
/// binding that reads one keeps working after the binding that first read it is gone.
#[derive(Clone, Copy)]
pub(crate) struct Signals {
    pub touched: Signal<bool>,
    pub blurred: Signal<bool>,
    pub dirty: Signal<bool>,
    pub default: Signal<bool>,
    pub validating: Signal<bool>,
    pub effective: Signal<ErrorMap>,
    pub value: Signal<u64>,
    pub shape: Signal<u64>,
}

impl Signals {
    pub fn new() -> Self {
        Self {
            touched: signal(false),
            blurred: signal(false),
            dirty: signal(false),
            default: signal(true),
            validating: signal(false),
            effective: signal(ErrorMap::default()),
            value: signal(0),
            shape: signal(0),
        }
    }

    /// Copies the entry's flags into the signals. A signal whose value did not change wakes nothing.
    pub fn apply(&self, entry: &Entry) {
        self.touched.set(entry.touched);
        self.blurred.set(entry.blurred);
        self.dirty.set(entry.dirty);
        self.validating.set(entry.is_validating());
        self.effective.set(entry.effective());
    }
}

/// One path's entry and its signals.
pub(crate) struct Node {
    pub entry: Entry,
    pub sig: Signals,
}

pub(crate) type Entries = HashMap<FieldPath, Node>;

/// Moves the metadata of array rows under `prefix` to the rows' new indices.
/// `remap` gives a row's new index, or `None` when the row is removed. This
/// keeps errors with their row, as `metaHelper.ts` does.
pub(crate) fn remap_rows<V>(
    entries: &mut HashMap<FieldPath, V>,
    prefix: &FieldPath,
    remap: impl Fn(usize) -> Option<usize>,
) {
    let depth = prefix.segments().len();
    let mut moves: Vec<(FieldPath, Option<FieldPath>)> = Vec::new();
    for key in entries.keys() {
        let segments = key.segments();
        if segments.len() <= depth || &segments[..depth] != prefix.segments() {
            continue;
        }
        let PathSegment::Index(row) = segments[depth] else {
            continue;
        };
        let new_row = remap(row);
        if new_row == Some(row) {
            continue;
        }
        let target = new_row.map(|index| {
            let mut moved = segments.to_vec();
            moved[depth] = PathSegment::Index(index);
            FieldPath::from_segments(&moved)
        });
        moves.push((key.clone(), target));
    }
    // Take every moved entry out before putting any back, so a row can take
    // the place of one that moved away.
    let mut taken = Vec::with_capacity(moves.len());
    for (old, target) in moves {
        if let Some(entry) = entries.remove(&old) {
            taken.push((target, entry));
        }
    }
    for (target, entry) in taken {
        if let Some(target) = target {
            entries.insert(target, entry);
        }
    }
}

/// The newest async run of one cause. Starting a run cancels the older task,
/// and an answer from an older generation is dropped.
#[derive(Default)]
pub(crate) struct Slot {
    generation: u64,
    task: Option<Spawned<()>>,
}

impl Slot {
    /// Cancels the running task and starts a new generation. Returns it.
    pub fn restart(&mut self) -> u64 {
        self.cancel();
        self.generation
    }

    /// Cancels the running task, so its answer is dropped.
    pub fn cancel(&mut self) {
        self.generation += 1;
        if let Some(task) = self.task.take() {
            task.cancel();
        }
    }

    pub fn is_current(&self, generation: u64) -> bool {
        self.generation == generation
    }

    pub fn set_task(&mut self, task: Spawned<()>) {
        self.task = Some(task);
    }
}

/// Which causes have an async validation in flight. TanStack's `validationMetaMap`, read per field.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ValidationMetaMap {
    pending: [bool; CAUSE_COUNT],
}

impl ValidationMetaMap {
    pub(crate) fn from_pending(pending: [bool; CAUSE_COUNT]) -> Self {
        Self { pending }
    }

    /// Whether an async validation of `cause` is running or waiting for its debounce.
    pub fn is_pending(&self, cause: crate::ValidationCause) -> bool {
        self.pending[cause.index()]
    }
}

/// A field's registration record, as TanStack's `getFieldInfo` returns it.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldInfo {
    /// The field's path.
    pub path: FieldPath,
    /// Whether the field is registered with the form. A field removed with `delete_field` is not.
    pub registered: bool,
    /// The async validations in flight.
    pub validation: ValidationMetaMap,
}

/// A field's stored metadata before the derived values are computed, as TanStack's
/// `fieldMetaBase` holds it: the flags, the field's own errors and the form's errors apart.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FieldMetaBase {
    /// Edited, blurred or submitted at least once since the last reset.
    pub is_touched: bool,
    /// Blurred at least once since the last reset.
    pub is_blurred: bool,
    /// Edited since the last reset.
    pub is_dirty: bool,
    /// The errors from the field's own validators.
    pub own_errors: ErrorMap,
    /// The errors from the form's validators.
    pub form_errors: ErrorMap,
    /// The async validations in flight.
    pub validation: ValidationMetaMap,
}
