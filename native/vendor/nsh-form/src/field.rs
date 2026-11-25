use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;
use std::time::Duration;

use gpui_form_core::FieldPath;
use nsh_core::prelude::*;

use crate::form::Form;
use crate::listeners::FieldListeners;
use crate::meta::{Entry, FieldInfo, FieldMeta, FieldMetaBase, Signals, Slot, ValidationMetaMap};
use crate::validation::{
    AsyncRule, CAUSE_COUNT, ErrorMap, Event, Job, Rule, ValidationCause, ValidationError, boxed,
};

/// The getter and setter of one field, as shared closures so that an array
/// row can capture its key.
/// Reads a value out of the form's state, or `None` when the state has none at that path.
pub(crate) type ReadFn<H, T> = Rc<dyn for<'a> Fn(&'a H) -> Option<&'a T>>;

/// Writes to a value in the form's state, or `None` when the state has none at that path.
pub(crate) type WriteFn<H, T> = Rc<dyn for<'a> Fn(&'a mut H) -> Option<&'a mut T>>;

pub(crate) struct Access<H, T> {
    /// The value at this path, or `None` when the path has no value in this state (an array
    /// row that is not in the baseline, for example).
    pub get: ReadFn<H, T>,
    pub get_mut: WriteFn<H, T>,
}

impl<H, T> Access<H, T> {
    /// The value. Panics when the value is missing.
    pub(crate) fn read<'a>(&self, values: &'a H) -> &'a T {
        (self.get)(values).expect("the array row of this field was removed")
    }

    /// The value, if the state has one at this path.
    pub(crate) fn read_opt<'a>(&self, values: &'a H) -> Option<&'a T> {
        (self.get)(values)
    }

    /// The value for writing. Panics when the value is missing.
    pub(crate) fn write<'a>(&self, values: &'a mut H) -> &'a mut T {
        (self.get_mut)(values).expect("the array row of this field was removed")
    }
}

impl<H, T> Clone for Access<H, T> {
    fn clone(&self) -> Self {
        Self {
            get: self.get.clone(),
            get_mut: self.get_mut.clone(),
        }
    }
}

pub(crate) fn access<H: 'static, T: 'static>(
    get: impl Fn(&H) -> &T + 'static,
    get_mut: impl Fn(&mut H) -> &mut T + 'static,
) -> Access<H, T> {
    access_opt(
        move |values: &H| Some(get(values)),
        move |values: &mut H| Some(get_mut(values)),
    )
}

pub(crate) fn access_opt<H: 'static, T: 'static>(
    get: impl Fn(&H) -> Option<&T> + 'static,
    get_mut: impl Fn(&mut H) -> Option<&mut T> + 'static,
) -> Access<H, T> {
    Access {
        get: Rc::new(get),
        get_mut: Rc::new(get_mut),
    }
}

/// Where a field's value is in the form. `None` once an array row is removed.
pub(crate) type Locate = Rc<dyn Fn() -> Option<FieldPath>>;

/// The validators of one field. Attach them with [`Field::validators`], or use
/// the methods of [`Field`] directly.
pub struct FieldValidators<H, T> {
    pub(crate) sync: [Vec<Rule<H, T>>; CAUSE_COUNT],
    pub(crate) asynch: [Vec<AsyncRule<T>>; CAUSE_COUNT],
    pub(crate) debounce: [Option<Duration>; CAUSE_COUNT],
    pub(crate) listen_change: Vec<String>,
    pub(crate) listen_blur: Vec<String>,
}

impl<H, T> Default for FieldValidators<H, T> {
    fn default() -> Self {
        Self {
            sync: Default::default(),
            asynch: Default::default(),
            debounce: [None; CAUSE_COUNT],
            listen_change: Vec::new(),
            listen_blur: Vec::new(),
        }
    }
}

impl<H: 'static, T: 'static> FieldValidators<H, T> {
    /// No validators.
    pub fn new() -> Self {
        Self::default()
    }

    fn push_sync(&mut self, cause: ValidationCause, rule: Rule<H, T>) {
        self.sync[cause.index()].push(rule);
    }

    fn push_async<F, Fut>(&mut self, cause: ValidationCause, rule: F)
    where
        F: Fn(T) -> Fut + 'static,
        Fut: Future<Output = Option<ValidationError>> + 'static,
    {
        let rule: AsyncRule<T> = Rc::new(move |value| boxed(rule(value)));
        self.asynch[cause.index()].push(rule);
    }

    /// Checked on change, on blur and on submit. The first failing rule gives the message.
    pub fn rule(mut self, rule: impl Fn(&T, &H) -> Option<String> + 'static) -> Self {
        let rule: Rule<H, T> = Rc::new(rule);
        for cause in [
            ValidationCause::Change,
            ValidationCause::Blur,
            ValidationCause::Submit,
        ] {
            self.push_sync(cause, rule.clone());
        }
        self
    }

    /// Runs once when the field mounts.
    pub fn on_mount(mut self, rule: impl Fn(&T, &H) -> Option<String> + 'static) -> Self {
        self.push_sync(ValidationCause::Mount, Rc::new(rule));
        self
    }

    /// Runs on every change.
    pub fn on_change(mut self, rule: impl Fn(&T, &H) -> Option<String> + 'static) -> Self {
        self.push_sync(ValidationCause::Change, Rc::new(rule));
        self
    }

    /// Runs on blur.
    pub fn on_blur(mut self, rule: impl Fn(&T, &H) -> Option<String> + 'static) -> Self {
        self.push_sync(ValidationCause::Blur, Rc::new(rule));
        self
    }

    /// Runs on submit.
    pub fn on_submit(mut self, rule: impl Fn(&T, &H) -> Option<String> + 'static) -> Self {
        self.push_sync(ValidationCause::Submit, Rc::new(rule));
        self
    }

    /// Runs on revalidation, see [`crate::ValidationLogic::Revalidate`].
    pub fn on_dynamic(mut self, rule: impl Fn(&T, &H) -> Option<String> + 'static) -> Self {
        self.push_sync(ValidationCause::Dynamic, Rc::new(rule));
        self
    }

    /// An async check on change. It runs after the sync checks pass, and a newer edit drops its answer.
    pub fn on_change_async<F, Fut>(mut self, rule: F) -> Self
    where
        F: Fn(T) -> Fut + 'static,
        Fut: Future<Output = Option<ValidationError>> + 'static,
    {
        self.push_async(ValidationCause::Change, rule);
        self
    }

    /// Waits this long after the last edit before the change check runs.
    pub fn on_change_async_debounce(mut self, delay: Duration) -> Self {
        self.debounce[ValidationCause::Change.index()] = Some(delay);
        self
    }

    /// `on_change_async` with its debounce, in one call.
    pub fn async_rule<F, Fut>(self, delay: Duration, rule: F) -> Self
    where
        F: Fn(T) -> Fut + 'static,
        Fut: Future<Output = Option<ValidationError>> + 'static,
    {
        self.on_change_async(rule).on_change_async_debounce(delay)
    }

    /// An async check on blur.
    pub fn on_blur_async<F, Fut>(mut self, rule: F) -> Self
    where
        F: Fn(T) -> Fut + 'static,
        Fut: Future<Output = Option<ValidationError>> + 'static,
    {
        self.push_async(ValidationCause::Blur, rule);
        self
    }

    /// Waits this long before the blur check runs.
    pub fn on_blur_async_debounce(mut self, delay: Duration) -> Self {
        self.debounce[ValidationCause::Blur.index()] = Some(delay);
        self
    }

    /// An async check on submit. The submit checks run one after another.
    pub fn on_submit_async<F, Fut>(mut self, rule: F) -> Self
    where
        F: Fn(T) -> Fut + 'static,
        Fut: Future<Output = Option<ValidationError>> + 'static,
    {
        self.push_async(ValidationCause::Submit, rule);
        self
    }

    /// An async check on revalidation.
    pub fn on_dynamic_async<F, Fut>(mut self, rule: F) -> Self
    where
        F: Fn(T) -> Fut + 'static,
        Fut: Future<Output = Option<ValidationError>> + 'static,
    {
        self.push_async(ValidationCause::Dynamic, rule);
        self
    }

    /// Waits this long before the revalidation check runs.
    pub fn on_dynamic_async_debounce(mut self, delay: Duration) -> Self {
        self.debounce[ValidationCause::Dynamic.index()] = Some(delay);
        self
    }

    /// Checks the value with `schema` on change. The first issue for the value itself gives the message.
    pub fn on_change_schema(mut self, schema: impl crate::Schema<T> + 'static) -> Self {
        self.push_sync(ValidationCause::Change, schema_rule(schema));
        self
    }

    /// Checks the value with `schema` on blur.
    pub fn on_blur_schema(mut self, schema: impl crate::Schema<T> + 'static) -> Self {
        self.push_sync(ValidationCause::Blur, schema_rule(schema));
        self
    }

    /// Checks the value with `schema` on submit.
    pub fn on_submit_schema(mut self, schema: impl crate::Schema<T> + 'static) -> Self {
        self.push_sync(ValidationCause::Submit, schema_rule(schema));
        self
    }

    /// Revalidates this field when the fields at `paths` change. Paths are dotted: `"address.city"`.
    pub fn on_change_listen_to(mut self, paths: &[&str]) -> Self {
        self.listen_change
            .extend(paths.iter().map(|path| path.to_string()));
        self
    }

    /// Revalidates this field when the fields at `paths` are blurred.
    pub fn on_blur_listen_to(mut self, paths: &[&str]) -> Self {
        self.listen_blur
            .extend(paths.iter().map(|path| path.to_string()));
        self
    }

    /// Adds the validators of `other` to these.
    pub(crate) fn merged(mut self, other: Self) -> Self {
        for (mine, theirs) in self.sync.iter_mut().zip(other.sync) {
            mine.extend(theirs);
        }
        for (mine, theirs) in self.asynch.iter_mut().zip(other.asynch) {
            mine.extend(theirs);
        }
        for (mine, theirs) in self.debounce.iter_mut().zip(other.debounce) {
            if theirs.is_some() {
                *mine = theirs;
            }
        }
        self.listen_change.extend(other.listen_change);
        self.listen_blur.extend(other.listen_blur);
        self
    }
}

/// A validator rule that checks the value with `schema`, giving the first issue's message.
fn schema_rule<H: 'static, T: 'static>(schema: impl crate::Schema<T> + 'static) -> Rule<H, T> {
    let schema: Rc<dyn crate::Schema<T>> = Rc::new(schema);
    Rc::new(move |value: &T, _: &H| crate::schema::first_message(schema.as_ref(), value))
}

pub(crate) struct Inner<H, T> {
    access: Access<H, T>,
    locate: Locate,
    shape: Option<fn(&T) -> usize>,
    last: RefCell<Option<T>>,
    last_len: RefCell<Option<usize>>,
    validators: RefCell<FieldValidators<H, T>>,
    listeners: RefCell<FieldListeners<T>>,
    slots: RefCell<[Slot; CAUSE_COUNT]>,
    change_listener: RefCell<Slot>,
    input: RefCell<Option<ElementRef>>,
}

/// One value of a form. `Copy`. Reads are tracked, and each one wakes only for its own value.
/// Make one in the component that shows it with [`crate::field!`].
pub struct Field<H: 'static, T: 'static> {
    pub(crate) form: Form<H>,
    inner: Stored<Rc<Inner<H, T>>>,
}

impl<H, T> Clone for Field<H, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<H, T> Copy for Field<H, T> {}

/// What the form needs from a field, whatever its value type.
pub(crate) trait AnyField<H: 'static> {
    /// Identifies the field. Two handles to one field have the same key.
    fn key(&self) -> usize;
    fn path(&self) -> Option<FieldPath>;
    /// The field's entry, read without tracking.
    fn entry(&self) -> Entry;
    /// The value, boxed, tracked. [`Form::get_field_value`] downcasts it.
    fn value_any(&self) -> Box<dyn std::any::Any>;
    /// Brings the field's value and shape signals up to date with the form's values.
    fn publish(&self);
    /// Runs the sync validators of `causes`, stores their errors, and reports whether one failed.
    fn sync(&self, causes: &[ValidationCause], values: &H) -> bool;
    fn has_async(&self, cause: ValidationCause) -> bool;
    fn job(&self, cause: ValidationCause, values: &H) -> Option<Job>;
    fn begin_async(&self, cause: ValidationCause) -> u64;
    fn finish_async(
        &self,
        cause: ValidationCause,
        generation: u64,
        answer: Option<ValidationError>,
    );
    fn cancel_async(&self, cause: ValidationCause);
    fn start_async_event(&self, cause: ValidationCause, values: &H);
    fn listens_to(&self, cause: ValidationCause, source: &str) -> bool;
    fn invalid(&self) -> bool;
    fn focus(&self);
    fn set_from_form(&self, cause: ValidationCause, message: Option<ValidationError>);
    fn set_server(&self, message: ValidationError);
    fn mark_touched(&self);
    fn snapshot(&self) -> FieldMeta;
    fn fire_submit(&self);
    fn reset_value(&self);
    fn cancel_all(&self);
    fn validation(&self) -> ValidationMetaMap;
    fn meta_base(&self) -> FieldMetaBase;
}

impl<H: Clone + PartialEq + 'static, T: Clone + PartialEq + 'static> Field<H, T> {
    pub(crate) fn new(
        form: Form<H>,
        access: Access<H, T>,
        locate: Locate,
        shape: Option<fn(&T) -> usize>,
    ) -> Self {
        let inner = Rc::new(Inner {
            access,
            locate,
            shape,
            last: RefCell::new(None),
            last_len: RefCell::new(None),
            validators: RefCell::new(FieldValidators::default()),
            listeners: RefCell::new(FieldListeners::default()),
            slots: RefCell::new(Default::default()),
            change_listener: RefCell::new(Slot::default()),
            input: RefCell::new(None),
        });
        Self {
            form,
            inner: stored(inner),
        }
    }

    fn inner(&self) -> Rc<Inner<H, T>> {
        self.inner.get()
    }

    fn as_dyn(&self) -> Rc<dyn AnyField<H>> {
        Rc::new(*self)
    }

    /// The access closures of this field, shared with array rows.
    pub(crate) fn access(&self) -> Access<H, T> {
        self.inner().access.clone()
    }

    /// The signals of this field's path, or `None` once its array row was removed.
    fn sig(&self) -> Option<Signals> {
        self.try_path().map(|path| self.form.signals_for(&path))
    }

    /// The field's path in the form, such as `"address.city"`. Panics when the
    /// field is an array row that was removed.
    pub fn path(&self) -> FieldPath {
        self.try_path()
            .expect("the array row of this field was removed")
    }

    pub(crate) fn try_path(&self) -> Option<FieldPath> {
        (self.inner().locate)()
    }

    /// The value. Tracked: it wakes only when this value changes.
    pub fn value(&self) -> T {
        if let Some(sig) = self.sig() {
            sig.value.get();
        }
        self.value_untracked()
    }

    /// The value, without tracking.
    pub fn value_untracked(&self) -> T {
        let inner = self.inner();
        self.form
            .state
            .with(|state| inner.access.read(state.current()).clone())
    }

    /// The value. Tracked. Same as [`Field::value`].
    pub fn get_value(&self) -> T {
        self.value()
    }

    /// Runs `read` on the value without tracking.
    pub(crate) fn with_value_untracked<R>(&self, read: impl FnOnce(&T) -> R) -> R {
        let inner = self.inner();
        self.form
            .state
            .with(|state| read(inner.access.read(state.current())))
    }

    /// Subscribes to the row count of an array field. Tracked.
    pub(crate) fn track_shape(&self) {
        if let Some(sig) = self.sig() {
            sig.shape.get();
        }
    }

    /// Wakes the subscribers of the row list, for an operation that reordered it. Array operations use it.
    pub(crate) fn bump_shape(&self) {
        if let Some(sig) = self.sig() {
            sig.shape.update(|shape| *shape = shape.wrapping_add(1));
        }
    }

    /// Writes the value without marking the field changed. Array operations use it.
    pub(crate) fn update_value(&self, change: impl FnOnce(&mut T)) {
        let inner = self.inner();
        self.form
            .state
            .with(|state| change(inner.access.write(state.current_mut())));
        // The form saves before it publishes. A field removed from the registry publishes after, itself.
        self.form.bump();
        self.publish();
    }

    /// Sets the value, as TanStack `setFieldValue` does: the field is marked
    /// touched and dirty even when the value is equal, and validators run.
    pub fn set_value(&self, value: T) {
        self.update_value(|slot| *slot = value);
        self.changed(Vec::new());
    }

    /// Same as [`Field::set_value`], under the TanStack name `handleChange`.
    pub fn handle_change(&self, value: T) {
        self.set_value(value);
    }

    /// Same as [`Field::set_value`], except that an equal value changes nothing.
    pub fn set(&self, value: T) {
        let inner = self.inner();
        if self
            .form
            .state
            .with(|state| inner.access.read(state.current()) == &value)
        {
            return;
        }
        self.set_value(value);
    }

    pub(crate) fn changed(&self, extra: Vec<Rc<dyn AnyField<H>>>) {
        self.with_entry(|entry| {
            entry.touched = true;
            entry.dirty = true;
            entry.own.set(ValidationCause::Mount, None);
            entry.from_form.set(ValidationCause::Mount, None);
        });
        self.fire_change_listeners();
        self.form.fire_form_change();
        let targets = self.targets(ValidationCause::Change, extra);
        self.form.run_event(Event::Change, &targets, true);
    }

    /// Marks the field blurred and runs the blur validators and listeners.
    /// Same as [`Field::handle_blur`], kept for existing callers.
    pub fn blur(&self) {
        self.handle_blur();
    }

    /// Marks the field touched and blurred, then runs the blur validators, the
    /// blur listeners, and the validators of the fields that listen to this one.
    pub fn handle_blur(&self) {
        self.with_entry(|entry| {
            entry.touched = true;
            entry.blurred = true;
        });
        let targets = self.targets(ValidationCause::Blur, Vec::new());
        self.form.run_event(Event::Blur, &targets, true);
        self.fire_blur_listeners();
        self.form.fire_form_blur();
    }

    /// Runs the mount validators and the mount listener.
    pub fn mount(&self) {
        let targets = vec![self.as_dyn()];
        self.form.run_event(Event::Mount, &targets, false);
        self.mount_listener();
    }

    /// Runs the validators of one cause on this field and returns its messages.
    /// Does nothing until the field is touched, except for mount.
    pub fn validate(&self, cause: ValidationCause) -> Vec<ValidationError> {
        if cause != ValidationCause::Mount && !self.is_touched_untracked() {
            return Vec::new();
        }
        let targets = self.targets(cause, Vec::new());
        self.form.run_event(Event::of_cause(cause), &targets, true);
        self.messages()
    }

    /// Attaches validators. They are added to the ones already on the field.
    pub fn validators(self, validators: FieldValidators<H, T>) -> Self {
        self.edit(|mine| mine.merged(validators))
    }

    /// Attaches listeners. A listener that is set replaces the one before it.
    /// The mount listener runs once here.
    pub fn listeners(self, listeners: FieldListeners<T>) -> Self {
        {
            let inner = self.inner();
            let mut current = inner.listeners.borrow_mut();
            if listeners.on_change.is_some() {
                current.on_change = listeners.on_change;
            }
            if listeners.on_blur.is_some() {
                current.on_blur = listeners.on_blur;
            }
            if listeners.on_mount.is_some() {
                current.on_mount = listeners.on_mount;
            }
            if listeners.on_submit.is_some() {
                current.on_submit = listeners.on_submit;
            }
            if listeners.on_change_debounce > Duration::ZERO {
                current.on_change_debounce = listeners.on_change_debounce;
            }
        }
        self.mount_listener();
        self
    }

    fn mount_listener(&self) {
        let listener = self.inner().listeners.borrow().on_mount.clone();
        if let Some(listener) = listener {
            listener(&self.value_untracked());
        }
    }

    fn edit(self, change: impl FnOnce(FieldValidators<H, T>) -> FieldValidators<H, T>) -> Self {
        let inner = self.inner();
        let current = std::mem::take(&mut *inner.validators.borrow_mut());
        *inner.validators.borrow_mut() = change(current);
        self
    }

    /// Checked on change, on blur and on submit. The first failing rule gives the message.
    pub fn rule(self, rule: impl Fn(&T, &H) -> Option<String> + 'static) -> Self {
        self.edit(|v| v.rule(rule))
    }

    /// An async check on change, with its debounce. See [`FieldValidators::async_rule`].
    pub fn async_rule<Fut>(self, debounce: Duration, rule: impl Fn(T) -> Fut + 'static) -> Self
    where
        Fut: Future<Output = Option<ValidationError>> + 'static,
    {
        self.edit(|v| v.async_rule(debounce, rule))
    }

    /// Runs on mount. Adding a mount rule runs the mount rules at once, and
    /// [`Field::mount`] runs them again.
    pub fn on_mount(self, rule: impl Fn(&T, &H) -> Option<String> + 'static) -> Self {
        let field = self.edit(|v| v.on_mount(rule));
        field.mount();
        field
    }

    /// Runs on every change.
    pub fn on_change(self, rule: impl Fn(&T, &H) -> Option<String> + 'static) -> Self {
        self.edit(|v| v.on_change(rule))
    }

    /// Runs on blur.
    pub fn on_blur(self, rule: impl Fn(&T, &H) -> Option<String> + 'static) -> Self {
        self.edit(|v| v.on_blur(rule))
    }

    /// Runs on submit.
    pub fn on_submit(self, rule: impl Fn(&T, &H) -> Option<String> + 'static) -> Self {
        self.edit(|v| v.on_submit(rule))
    }

    /// An async check on change.
    pub fn on_change_async<F, Fut>(self, rule: F) -> Self
    where
        F: Fn(T) -> Fut + 'static,
        Fut: Future<Output = Option<ValidationError>> + 'static,
    {
        self.edit(|v| v.on_change_async(rule))
    }

    /// The debounce of the change check.
    pub fn on_change_async_debounce(self, delay: Duration) -> Self {
        self.edit(|v| v.on_change_async_debounce(delay))
    }

    /// An async check on blur.
    pub fn on_blur_async<F, Fut>(self, rule: F) -> Self
    where
        F: Fn(T) -> Fut + 'static,
        Fut: Future<Output = Option<ValidationError>> + 'static,
    {
        self.edit(|v| v.on_blur_async(rule))
    }

    /// The debounce of the blur check.
    pub fn on_blur_async_debounce(self, delay: Duration) -> Self {
        self.edit(|v| v.on_blur_async_debounce(delay))
    }

    /// An async check on submit.
    pub fn on_submit_async<F, Fut>(self, rule: F) -> Self
    where
        F: Fn(T) -> Fut + 'static,
        Fut: Future<Output = Option<ValidationError>> + 'static,
    {
        self.edit(|v| v.on_submit_async(rule))
    }

    /// Checks the value with `schema` on change. See [`FieldValidators::on_change_schema`].
    pub fn on_change_schema(self, schema: impl crate::Schema<T> + 'static) -> Self {
        self.edit(|v| v.on_change_schema(schema))
    }

    /// Checks the value with `schema` on blur.
    pub fn on_blur_schema(self, schema: impl crate::Schema<T> + 'static) -> Self {
        self.edit(|v| v.on_blur_schema(schema))
    }

    /// Checks the value with `schema` on submit.
    pub fn on_submit_schema(self, schema: impl crate::Schema<T> + 'static) -> Self {
        self.edit(|v| v.on_submit_schema(schema))
    }

    /// Checks the current value against `schema` now. Issues become this field's submit error,
    /// and the result gives them back.
    pub fn parse_value_with_schema(
        &self,
        schema: &dyn crate::Schema<T>,
    ) -> Result<T, Vec<crate::SchemaIssue>> {
        let value = self.value_untracked();
        let issues = schema.check(&value);
        self.apply_issues(&issues);
        if issues.is_empty() {
            Ok(value)
        } else {
            Err(issues)
        }
    }

    /// The async form of [`Field::parse_value_with_schema`].
    pub async fn parse_value_with_schema_async<S: crate::AsyncSchema<T>>(
        &self,
        schema: &S,
    ) -> Result<T, Vec<crate::SchemaIssue>> {
        let value = self.value_untracked();
        let issues = schema.check(value.clone()).await;
        self.apply_issues(&issues);
        if issues.is_empty() {
            Ok(value)
        } else {
            Err(issues)
        }
    }

    fn apply_issues(&self, issues: &[crate::SchemaIssue]) {
        let message = issues.first().map(|issue| issue.message.clone());
        self.with_entry(|entry| entry.own.set(ValidationCause::Submit, message));
    }

    /// Revalidates this field when the fields at `paths` change.
    pub fn on_change_listen_to(self, paths: &[&str]) -> Self {
        self.edit(|v| v.on_change_listen_to(paths))
    }

    /// Revalidates this field when the fields at `paths` are blurred.
    pub fn on_blur_listen_to(self, paths: &[&str]) -> Self {
        self.edit(|v| v.on_blur_listen_to(paths))
    }

    /// Whether the field was edited, blurred or submitted since the last reset. Tracked.
    pub fn is_touched(&self) -> bool {
        self.sig().is_some_and(|sig| sig.touched.get())
    }

    fn is_touched_untracked(&self) -> bool {
        self.read_entry().touched
    }

    /// Whether the field was blurred since the last reset. Tracked.
    pub fn is_blurred(&self) -> bool {
        self.sig().is_some_and(|sig| sig.blurred.get())
    }

    /// Whether the field was edited since the last reset. Stays true after the
    /// value returns to its default. Tracked. TanStack's `isDirty`.
    pub fn was_changed(&self) -> bool {
        self.sig().is_some_and(|sig| sig.dirty.get())
    }

    /// Whether the field was not edited since the last reset. Tracked.
    pub fn is_pristine(&self) -> bool {
        !self.was_changed()
    }

    /// Whether the value equals the baseline value. Tracked.
    pub fn is_default_value(&self) -> bool {
        self.sig().is_none_or(|sig| sig.default.get())
    }

    /// Whether no cause has an error. Tracked.
    pub fn is_valid(&self) -> bool {
        self.sig().is_none_or(|sig| sig.effective.get().is_empty())
    }

    /// The field's errors, keyed by cause. Tracked.
    pub fn error_map(&self) -> ErrorMap {
        self.sig()
            .map(|sig| sig.effective.get())
            .unwrap_or_default()
    }

    /// The distinct error messages. Tracked.
    pub fn errors(&self) -> Vec<ValidationError> {
        self.error_map().messages()
    }

    /// The field's metadata as a snapshot. Tracked.
    pub fn meta(&self) -> FieldMeta {
        self.snapshot_meta()
    }

    /// A memo of part of the metadata. A binding that reads it reruns only when that part changes.
    pub fn select<R: PartialEq + 'static>(
        &self,
        pick: impl Fn(&FieldMeta) -> R + 'static,
    ) -> Memo<R> {
        let field = *self;
        computed(move || pick(&field.meta()))
    }

    /// The message to show, if any. Errors show once the field was blurred or
    /// the form was submitted. A server error and a mount error show at once.
    pub fn error(&self) -> Option<String> {
        let sig = self.sig()?;
        let effective = sig.effective.get();
        if self.form.submitted.get() || sig.blurred.get() {
            effective.first()
        } else {
            effective
                .get(ValidationCause::Server)
                .or(effective.get(ValidationCause::Mount))
                .map(str::to_owned)
        }
    }

    /// Same as [`Field::is_blurred`], kept for existing callers.
    pub fn touched(&self) -> bool {
        self.is_blurred()
    }

    /// Whether this value differs from the baseline. Kept for existing callers. Tracked.
    pub fn dirty(&self) -> bool {
        self.sig().is_some_and(|sig| !sig.default.get())
    }

    /// Whether an async validator is running or waiting for its debounce. Tracked.
    pub fn is_validating(&self) -> bool {
        self.sig().is_some_and(|sig| sig.validating.get())
    }

    /// Replaces the field's errors with `map`. Use it to show an error the field did not compute.
    pub fn set_error_map(&self, map: ErrorMap) {
        self.with_entry(|entry| entry.own = map);
    }

    /// Changes the field's metadata. The touched, blurred and changed flags and the
    /// error map are written back.
    pub fn set_meta(&self, change: impl FnOnce(&mut FieldMeta)) {
        let mut meta = self.meta();
        change(&mut meta);
        self.with_entry(|entry| {
            entry.touched = meta.is_touched;
            entry.blurred = meta.is_blurred;
            entry.dirty = meta.was_changed;
            entry.own = meta.error_map;
        });
    }

    /// Sets the value to the default of its type and clears the field's metadata.
    /// A typed field cannot be absent, so the default stands in for removal.
    pub fn delete_value(&self)
    where
        T: Default,
    {
        self.update_value(|slot| *slot = T::default());
        self.reset_meta();
    }

    /// Replaces the field's validators and listeners at run time, as TanStack's `fieldApi.update`.
    pub fn update(&self, validators: FieldValidators<H, T>, listeners: FieldListeners<T>) {
        let inner = self.inner();
        *inner.validators.borrow_mut() = validators;
        *inner.listeners.borrow_mut() = listeners;
    }

    /// The value, if it differs from the baseline: what a patch sends for this field. Tracked.
    pub fn changed_value(&self) -> Option<T> {
        if self.is_default_value() {
            None
        } else {
            Some(self.value())
        }
    }

    /// The field's registration record, as TanStack's `getInfo`. Tracked: it wakes when the
    /// field's async validations change.
    pub fn get_info(&self) -> FieldInfo {
        FieldInfo {
            path: self.path(),
            registered: self.is_registered(),
            validation: self.validation_meta_map(),
        }
    }

    /// Whether the form lists this field. A field removed with `delete_field` is not listed.
    fn is_registered(&self) -> bool {
        let key = self.key();
        self.form
            .registered()
            .iter()
            .any(|field| field.key() == key)
    }

    /// Which causes have an async validation in flight. Tracked.
    pub fn validation_meta_map(&self) -> ValidationMetaMap {
        if let Some(sig) = self.sig() {
            sig.validating.get();
        }
        ValidationMetaMap::from_pending(self.read_entry().pending)
    }

    /// The fields that listen to this one for `cause`, by path. TanStack's `getLinkedFields`.
    pub fn get_linked_fields(&self, cause: ValidationCause) -> Vec<FieldPath> {
        let Some(source) = self.try_path().map(|path| path.to_string()) else {
            return Vec::new();
        };
        self.form
            .registered()
            .into_iter()
            .filter(|field| field.listens_to(cause, &source))
            .filter_map(|field| field.path())
            .collect()
    }

    /// The field's metadata before the derived values, as TanStack's `fieldMetaBase`. Tracked.
    pub fn meta_base(&self) -> FieldMetaBase {
        let effective = match self.sig() {
            Some(sig) => {
                sig.effective.get();
                sig.touched.get();
                sig.blurred.get();
                sig.dirty.get();
                self.read_entry()
            }
            None => self.read_entry(),
        };
        FieldMetaBase {
            is_touched: effective.touched,
            is_blurred: effective.blurred,
            is_dirty: effective.dirty,
            own_errors: effective.own.clone(),
            form_errors: effective.from_form.clone(),
            validation: ValidationMetaMap::from_pending(effective.pending),
        }
    }

    /// Focuses the bound input, if there is one.
    pub fn focus(&self) {
        let input = *self.inner().input.borrow();
        if let Some(input) = input {
            input.focus();
        }
    }

    pub(crate) fn bind_input(&self, reference: ElementRef) {
        *self.inner().input.borrow_mut() = Some(reference);
    }

    /// The fields to validate for an event on this field: this field, the
    /// fields that listen to it for `cause`, and `extra`.
    fn targets(
        &self,
        cause: ValidationCause,
        extra: Vec<Rc<dyn AnyField<H>>>,
    ) -> Vec<Rc<dyn AnyField<H>>> {
        let key = self.key();
        let source = self.try_path().map(|path| path.to_string());
        let mut out: Vec<Rc<dyn AnyField<H>>> = Vec::new();
        for field in self.form.registered() {
            let linked = source
                .as_deref()
                .is_some_and(|source| field.listens_to(cause, source));
            if (field.key() == key || linked) && !out.iter().any(|seen| seen.key() == field.key()) {
                out.push(field);
            }
        }
        if !out.iter().any(|seen| seen.key() == key) {
            out.push(self.as_dyn());
        }
        for field in extra {
            if !out.iter().any(|seen| seen.key() == field.key()) {
                out.push(field);
            }
        }
        out
    }

    /// The entry, read without tracking. Logic reads go through here.
    pub(crate) fn read_entry(&self) -> Entry {
        match self.try_path() {
            Some(path) => self.form.entries.with(|entries| {
                entries
                    .get(&path)
                    .map(|node| node.entry.clone())
                    .unwrap_or_default()
            }),
            None => Entry::default(),
        }
    }

    /// Changes the entry, then brings the bindings that read it up to date.
    fn with_entry(&self, change: impl FnOnce(&mut Entry)) {
        if let Some(path) = self.try_path() {
            self.form.signals_for(&path);
            self.form.entries.with(|entries| {
                if let Some(node) = entries.get_mut(&path) {
                    change(&mut node.entry);
                }
            });
            self.form.bump();
        }
    }

    fn reset_meta(&self) {
        self.cancel_all();
        self.with_entry(|entry| *entry = Entry::default());
    }

    fn messages(&self) -> Vec<ValidationError> {
        self.read_entry().effective().messages()
    }

    fn snapshot_meta(&self) -> FieldMeta {
        match self.sig() {
            Some(sig) => FieldMeta {
                is_touched: sig.touched.get(),
                is_blurred: sig.blurred.get(),
                was_changed: sig.dirty.get(),
                is_default_value: sig.default.get(),
                is_validating: sig.validating.get(),
                error_map: sig.effective.get(),
            },
            None => FieldMeta {
                is_touched: false,
                is_blurred: false,
                was_changed: false,
                is_default_value: true,
                is_validating: false,
                error_map: ErrorMap::default(),
            },
        }
    }

    fn fire_change_listeners(&self) {
        let inner = self.inner();
        let (listener, delay) = {
            let listeners = inner.listeners.borrow();
            (listeners.on_change.clone(), listeners.on_change_debounce)
        };
        let Some(listener) = listener else {
            return;
        };
        let value = self.value_untracked();
        if delay.is_zero() {
            listener(&value);
            return;
        }
        let generation = inner.change_listener.borrow_mut().restart();
        let field = *self;
        let task = spawn(async move {
            timer(delay).await;
            if field
                .inner()
                .change_listener
                .borrow()
                .is_current(generation)
            {
                listener(&value);
            }
        });
        inner.change_listener.borrow_mut().set_task(task);
    }

    fn fire_blur_listeners(&self) {
        let listener = self.inner().listeners.borrow().on_blur.clone();
        if let Some(listener) = listener {
            listener(&self.value_untracked());
        }
    }

    fn sync_causes(&self, causes: &[ValidationCause], values: &H) -> bool {
        let inner = self.inner();
        let value = inner.access.read(values).clone();
        let mut messages = Vec::with_capacity(causes.len());
        for cause in causes {
            let rules = inner.validators.borrow().sync[cause.index()].clone();
            let message = rules.iter().find_map(|rule| rule(&value, values));
            messages.push((*cause, message));
        }
        let mut failed = false;
        self.with_entry(|entry| {
            for (cause, message) in messages {
                entry.own.set(cause, message);
            }
            let effective = entry.effective();
            failed = causes.iter().any(|cause| effective.get(*cause).is_some());
        });
        failed
    }

    fn async_debounce(&self, cause: ValidationCause) -> Duration {
        let own = self.inner().validators.borrow().debounce[cause.index()];
        own.unwrap_or_else(|| self.form.async_debounce_default())
    }

    fn job_for(&self, cause: ValidationCause, values: &H) -> Option<Job> {
        let inner = self.inner();
        let rules = {
            let validators = inner.validators.borrow();
            if validators.asynch[cause.index()].is_empty() {
                return None;
            }
            validators.asynch[cause.index()].clone()
        };
        let delay = self.async_debounce(cause);
        let value = inner.access.read(values).clone();
        let job: Job = Box::pin(async move {
            if !delay.is_zero() {
                timer(delay).await;
            }
            for rule in rules {
                if let Some(message) = rule(value.clone()).await {
                    return Some(message);
                }
            }
            None
        });
        Some(job)
    }

    fn start_async(&self, cause: ValidationCause, values: &H) {
        let Some(job) = self.job_for(cause, values) else {
            return;
        };
        let generation = self.begin_async(cause);
        let field = *self;
        let task = spawn(async move {
            let answer = job.await;
            field.finish_async(cause, generation, answer);
        });
        self.inner().slots.borrow_mut()[cause.index()].set_task(task);
    }
}

impl<H: Clone + PartialEq + 'static, T: Clone + PartialEq + 'static> AnyField<H> for Field<H, T> {
    fn key(&self) -> usize {
        Rc::as_ptr(&self.inner()) as usize
    }

    fn path(&self) -> Option<FieldPath> {
        self.try_path()
    }

    fn entry(&self) -> Entry {
        self.read_entry()
    }

    fn value_any(&self) -> Box<dyn std::any::Any> {
        Box::new(self.value())
    }

    fn publish(&self) {
        let Some(path) = self.try_path() else {
            return;
        };
        let sig = self.form.signals_for(&path);
        let inner = self.inner();
        let (value, default, len) = self.form.state.with(|state| {
            let current = inner.access.read(state.current());
            let default = inner
                .access
                .read_opt(state.baseline())
                .is_some_and(|baseline| baseline == current);
            (
                current.clone(),
                default,
                inner.shape.map(|shape| shape(current)),
            )
        });
        let changed = inner.last.borrow().as_ref() != Some(&value);
        if changed {
            *inner.last.borrow_mut() = Some(value);
            sig.value.update(|tick| *tick = tick.wrapping_add(1));
        }
        sig.default.set(default);
        if let Some(len) = len {
            let previous = inner.last_len.replace(Some(len));
            if previous.is_some_and(|previous| previous != len) {
                sig.shape.update(|shape| *shape = shape.wrapping_add(1));
            }
        }
    }

    fn sync(&self, causes: &[ValidationCause], values: &H) -> bool {
        self.sync_causes(causes, values)
    }

    fn has_async(&self, cause: ValidationCause) -> bool {
        !self.inner().validators.borrow().asynch[cause.index()].is_empty()
    }

    fn job(&self, cause: ValidationCause, values: &H) -> Option<Job> {
        self.job_for(cause, values)
    }

    fn begin_async(&self, cause: ValidationCause) -> u64 {
        let generation = self.inner().slots.borrow_mut()[cause.index()].restart();
        self.with_entry(|entry| entry.pending[cause.index()] = true);
        generation
    }

    fn finish_async(
        &self,
        cause: ValidationCause,
        generation: u64,
        answer: Option<ValidationError>,
    ) {
        if !self.inner().slots.borrow()[cause.index()].is_current(generation) {
            return;
        }
        self.with_entry(|entry| {
            entry.own.set(cause, answer);
            entry.pending[cause.index()] = false;
        });
    }

    fn cancel_async(&self, cause: ValidationCause) {
        self.inner().slots.borrow_mut()[cause.index()].cancel();
        self.with_entry(|entry| entry.pending[cause.index()] = false);
    }

    fn start_async_event(&self, cause: ValidationCause, values: &H) {
        self.start_async(cause, values);
    }

    fn listens_to(&self, cause: ValidationCause, source: &str) -> bool {
        let inner = self.inner();
        let validators = inner.validators.borrow();
        match cause {
            ValidationCause::Change => validators.listen_change.iter().any(|path| path == source),
            ValidationCause::Blur => validators.listen_blur.iter().any(|path| path == source),
            _ => false,
        }
    }

    fn invalid(&self) -> bool {
        !self.read_entry().effective().is_empty()
    }

    fn focus(&self) {
        Field::focus(self);
    }

    fn set_from_form(&self, cause: ValidationCause, message: Option<ValidationError>) {
        self.with_entry(|entry| entry.from_form.set(cause, message));
    }

    fn set_server(&self, message: ValidationError) {
        self.with_entry(|entry| entry.own.set(ValidationCause::Server, Some(message)));
    }

    fn mark_touched(&self) {
        self.with_entry(|entry| entry.touched = true);
    }

    fn snapshot(&self) -> FieldMeta {
        self.snapshot_meta()
    }

    fn fire_submit(&self) {
        let listener = self.inner().listeners.borrow().on_submit.clone();
        if let Some(listener) = listener {
            listener(&self.value_untracked());
        }
    }

    fn reset_value(&self) {
        let inner = self.inner();
        self.form.state.with(|state| {
            if let Some(baseline) = inner.access.read_opt(state.baseline()).cloned() {
                *inner.access.write(state.current_mut()) = baseline;
            }
        });
        self.reset_meta();
        self.form.bump();
    }

    fn validation(&self) -> ValidationMetaMap {
        self.validation_meta_map()
    }

    fn meta_base(&self) -> FieldMetaBase {
        Field::meta_base(self)
    }

    fn cancel_all(&self) {
        for slot in self.inner().slots.borrow_mut().iter_mut() {
            slot.cancel();
        }
        self.inner().change_listener.borrow_mut().cancel();
    }
}
