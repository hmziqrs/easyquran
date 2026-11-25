use std::cmp::Ordering;
use std::rc::Rc;

use gpui_form_core::FieldPath;
use nsh_core::prelude::*;

use crate::field::{Access, AnyField, Field, Locate, access, access_opt};
use crate::form::Form;
use crate::validation::{Event, ValidationCause};

/// The keys of an array's rows, in row order. A key stays with its row through
/// moves and removals, so a row's field keeps pointing at that row.
#[derive(Clone, Default)]
struct Rows {
    keys: Vec<u64>,
    next: u64,
}

/// Brings the keys to `len` rows, minting keys for new rows and dropping the rest.
fn sync_keys(rows: &mut Rows, len: usize) {
    while rows.keys.len() < len {
        let fresh = rows.next;
        rows.next += 1;
        rows.keys.push(fresh);
    }
    rows.keys.truncate(len);
}

/// The row count of a list, watched by the array's shape signal. It takes the field's own
/// type, `Vec<I>`, because the field stores it as a plain function pointer.
#[allow(clippy::ptr_arg)]
fn vec_len<I>(items: &Vec<I>) -> usize {
    items.len()
}

fn move_index(row: usize, from: usize, to: usize) -> usize {
    if row == from {
        to
    } else if from < to && row > from && row <= to {
        row - 1
    } else if to < from && row >= to && row < from {
        row + 1
    } else {
        row
    }
}

/// A list field: a `Vec` of rows, with keyed rows and array operations. `Copy`.
pub struct FieldArray<H: 'static, I: 'static> {
    field: Field<H, Vec<I>>,
    rows: Stored<Rows>,
}

impl<H: 'static, I: 'static> Clone for FieldArray<H, I> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<H: 'static, I: 'static> Copy for FieldArray<H, I> {}

impl<H: Clone + PartialEq + 'static, I: Clone + PartialEq + 'static> FieldArray<H, I> {
    fn new(field: Field<H, Vec<I>>) -> Self {
        Self {
            field,
            rows: stored(Rows::default()),
        }
    }

    /// The number of rows. Tracked.
    pub fn len(&self) -> usize {
        self.field.track_shape();
        let len = self.field.with_value_untracked(|items| items.len());
        self.sync_rows(len);
        len
    }

    /// Whether there are no rows. Tracked.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The rows' keys, in row order. A key stays with its row through moves and removals. Tracked.
    pub fn keys(&self) -> Vec<u64> {
        let len = self.len();
        self.sync_rows(len);
        self.rows.get().keys
    }

    /// The rows. Tracked.
    pub fn items(&self) -> Vec<I> {
        self.field.value()
    }

    fn sync_rows(&self, len: usize) {
        self.rows.with(|rows| sync_keys(rows, len));
    }

    fn new_key(&self) -> u64 {
        self.rows.with(|rows| {
            let key = rows.next;
            rows.next += 1;
            key
        })
    }

    /// A field for the whole row at `index`. Its key is fixed when it is made, so it
    /// still points at its row after a move or a removal. Panics when `index` is out of range.
    pub fn item_field(&self, index: usize) -> Field<H, I> {
        self.row_field(index, None, |item: &I| item, |item: &mut I| item)
    }

    /// A field for one part of the row at `index`, such as its `name`. `name` is the
    /// path segment the part is named by in the form.
    pub fn field_of<T>(
        &self,
        index: usize,
        name: &'static str,
        get: impl Fn(&I) -> &T + 'static,
        get_mut: impl Fn(&mut I) -> &mut T + 'static,
    ) -> Field<H, T>
    where
        T: Clone + PartialEq + 'static,
    {
        self.row_field(index, Some(name), get, get_mut)
    }

    fn row_field<T>(
        &self,
        index: usize,
        name: Option<&'static str>,
        get: impl Fn(&I) -> &T + 'static,
        get_mut: impl Fn(&mut I) -> &mut T + 'static,
    ) -> Field<H, T>
    where
        T: Clone + PartialEq + 'static,
    {
        let len = self.len();
        assert!(index < len, "row {index} is out of range for {len} rows");
        let key = self.rows.get().keys[index];
        let rows = self.rows;
        let array: Access<H, Vec<I>> = self.field.access();
        let read_array = array.get.clone();
        let write_array = array.get_mut.clone();
        let row_access = access_opt::<H, T>(
            move |form: &H| {
                let row = rows.with(|rows| rows.keys.iter().position(|own| *own == key))?;
                read_array(form)?.get(row).map(&get)
            },
            move |form: &mut H| {
                let row = rows.with(|rows| rows.keys.iter().position(|own| *own == key))?;
                write_array(form)?.get_mut(row).map(&get_mut)
            },
        );
        let array_path = self.field.path();
        let locate: Locate = Rc::new(move || {
            let row = rows.with(|rows| rows.keys.iter().position(|own| *own == key))?;
            let path = array_path.clone().push_index(row);
            Some(match name {
                Some(name) => path.push_name(name),
                None => path,
            })
        });
        self.field.form.register(row_access, locate, None)
    }

    /// Appends a row. The row's fields start untouched.
    pub fn push_value(&self, value: I) {
        let before = self.len();
        self.field.update_value(|items| items.push(value));
        let key = self.new_key();
        self.rows.with(|rows| rows.keys.push(key));
        self.reshape();
        self.changed_at(before);
    }

    /// Inserts a row at `index`, moving the rows from `index` on down by one.
    pub fn insert_value(&self, index: usize, value: I) {
        let len = self.len();
        let index = index.min(len);
        self.remap(move |row| Some(if row >= index { row + 1 } else { row }));
        let key = self.new_key();
        self.rows.with(|rows| rows.keys.insert(index, key));
        self.field.update_value(|items| items.insert(index, value));
        self.reshape();
        self.changed_at(index);
    }

    /// Replaces the row at `index`. Does nothing when `index` is out of range.
    pub fn replace_value(&self, index: usize, value: I) {
        if index >= self.len() {
            return;
        }
        self.field.update_value(|items| items[index] = value);
        self.changed_at(index);
    }

    /// Removes the row at `index`. Does nothing when `index` is out of range.
    pub fn remove_value(&self, index: usize) {
        let len = self.len();
        if index >= len {
            return;
        }
        self.remap(move |row| match row.cmp(&index) {
            Ordering::Equal => None,
            Ordering::Greater => Some(row - 1),
            Ordering::Less => Some(row),
        });
        self.rows.with(|rows| {
            rows.keys.remove(index);
        });
        self.field.update_value(|items| {
            items.remove(index);
        });
        self.reshape();
        self.changed_at(index);
    }

    /// Swaps two rows. Does nothing when either index is out of range.
    pub fn swap_values(&self, a: usize, b: usize) {
        let len = self.len();
        if a >= len || b >= len || a == b {
            return;
        }
        self.remap(move |row| {
            Some(if row == a {
                b
            } else if row == b {
                a
            } else {
                row
            })
        });
        self.rows.with(|rows| rows.keys.swap(a, b));
        self.field.update_value(|items| items.swap(a, b));
        self.reshape();
        self.changed_at(a.min(b));
    }

    /// Moves the row at `from` to `to`, shifting the rows in between. Does nothing when either index is out of range.
    pub fn move_value(&self, from: usize, to: usize) {
        let len = self.len();
        if from >= len || to >= len || from == to {
            return;
        }
        self.remap(move |row| Some(move_index(row, from, to)));
        self.rows.with(|rows| {
            let key = rows.keys.remove(from);
            rows.keys.insert(to, key);
        });
        self.field.update_value(|items| {
            let item = items.remove(from);
            items.insert(to, item);
        });
        self.reshape();
        self.changed_at(from.min(to));
    }

    /// Removes every row.
    pub fn clear_values(&self) {
        self.remap(|_| None);
        self.rows.with(|rows| rows.keys.clear());
        self.field.update_value(|items| items.clear());
        self.reshape();
        self.changed_at(0);
    }

    /// Validates the rows from `index` on, for `cause`. Rows that were never touched are skipped.
    pub fn validate_from(&self, index: usize, cause: ValidationCause) {
        let mut rows = self.field.form.rows_from(&self.field.path(), index);
        rows.retain(|row| row.entry().touched);
        self.field
            .form
            .run_event(Event::of_cause(cause), &rows, true);
    }

    fn remap(&self, remap: impl Fn(usize) -> Option<usize>) {
        let prefix = self.field.path();
        self.field.form.remap_rows(&prefix, remap);
    }

    /// Brings the keys' subscribers up to date after the rows were reordered or resized.
    fn reshape(&self) {
        self.field.bump_shape();
    }

    /// Notifies the array of a change that began at row `from`, and revalidates the touched rows from there on.
    fn changed_at(&self, from: usize) {
        let prefix = self.field.path();
        let mut rows: Vec<Rc<dyn AnyField<H>>> = self.field.form.rows_from(&prefix, from);
        rows.retain(|row| row.entry().touched);
        self.field.changed(rows);
    }
}

impl<H: Clone + PartialEq + 'static> Form<H> {
    /// Registers a list field. Prefer the [`crate::array_field!`] macro, which writes this call.
    pub fn array_field<I, G, M>(
        &self,
        path: &[&'static str],
        get: G,
        get_mut: M,
    ) -> FieldArray<H, I>
    where
        I: Clone + PartialEq + 'static,
        G: Fn(&H) -> &Vec<I> + 'static,
        M: Fn(&mut H) -> &mut Vec<I> + 'static,
    {
        let path = FieldPath::new(path);
        let locate: Locate = Rc::new(move || Some(path.clone()));
        self.owner.run(|| {
            FieldArray::new(self.register(
                access(get, get_mut),
                locate,
                Some(vec_len::<I> as fn(&Vec<I>) -> usize),
            ))
        })
    }
}
