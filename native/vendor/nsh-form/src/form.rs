use std::any::Any;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::time::Duration;

use gpui_form_core::{FieldPath, FormState, PathSegment};
use nsh_core::prelude::*;

use crate::field::{Access, AnyField, Field, Locate, access};
use crate::listeners::{Callback, FormListeners};
use crate::meta::{Entries, Entry, FieldInfo, FieldMeta, FieldMetaBase, Node, Signals, Slot};
use crate::validation::{
    AsyncFormRule, CAUSE_COUNT, ErrorMap, Event, FormJob, FormRule, FormValidationResult,
    ValidationCause, ValidationError, ValidationLogic, boxed, plan,
};

/// What a failed submit reports: a message for the form, and messages for
/// fields, named by path. The same shape as [`FormValidationResult`].
pub type SubmitError = FormValidationResult;

/// What a save returns: `Ok` on success, or the errors to show.
pub type SubmitResult = Result<(), SubmitError>;

pub(crate) type SaveFn<H> = Box<dyn FnOnce(H) -> Pin<Box<dyn Future<Output = SubmitResult>>>>;
type SubmitFn<H> = Rc<dyn Fn(H) -> Pin<Box<dyn Future<Output = SubmitResult>>>>;

/// Validators of a form. They see the whole form. A result puts its messages
/// on fields by path, as TanStack's `{ fields }` does.
pub struct FormValidators<H> {
    sync: [Vec<FormRule<H>>; CAUSE_COUNT],
    asynch: [Vec<AsyncFormRule<H>>; CAUSE_COUNT],
    debounce: [Option<Duration>; CAUSE_COUNT],
}

impl<H> Default for FormValidators<H> {
    fn default() -> Self {
        Self {
            sync: Default::default(),
            asynch: Default::default(),
            debounce: [None; CAUSE_COUNT],
        }
    }
}

impl<H: 'static> FormValidators<H> {
    /// No validators.
    pub fn new() -> Self {
        Self::default()
    }

    fn push_sync(
        mut self,
        cause: ValidationCause,
        rule: impl Fn(&H) -> FormValidationResult + 'static,
    ) -> Self {
        self.sync[cause.index()].push(Rc::new(rule));
        self
    }

    fn push_async<F, Fut>(mut self, cause: ValidationCause, rule: F) -> Self
    where
        F: Fn(H) -> Fut + 'static,
        Fut: Future<Output = FormValidationResult> + 'static,
    {
        let rule: AsyncFormRule<H> = Rc::new(move |values: H| boxed(rule(values)));
        self.asynch[cause.index()].push(rule);
        self
    }

    /// Checks the whole value with `schema` on change of any field.
    pub fn on_change_schema(self, schema: impl crate::Schema<H> + 'static) -> Self {
        self.push_sync(ValidationCause::Change, move |values: &H| {
            crate::schema::to_result(schema.check(values))
        })
    }

    /// Checks the whole value with `schema` on blur of any field.
    pub fn on_blur_schema(self, schema: impl crate::Schema<H> + 'static) -> Self {
        self.push_sync(ValidationCause::Blur, move |values: &H| {
            crate::schema::to_result(schema.check(values))
        })
    }

    /// Checks the whole value with `schema` on submit.
    pub fn on_submit_schema(self, schema: impl crate::Schema<H> + 'static) -> Self {
        self.push_sync(ValidationCause::Submit, move |values: &H| {
            crate::schema::to_result(schema.check(values))
        })
    }

    /// Runs once when the form mounts.
    pub fn on_mount(self, rule: impl Fn(&H) -> FormValidationResult + 'static) -> Self {
        self.push_sync(ValidationCause::Mount, rule)
    }

    /// Runs on every change of any field.
    pub fn on_change(self, rule: impl Fn(&H) -> FormValidationResult + 'static) -> Self {
        self.push_sync(ValidationCause::Change, rule)
    }

    /// Runs on blur of any field.
    pub fn on_blur(self, rule: impl Fn(&H) -> FormValidationResult + 'static) -> Self {
        self.push_sync(ValidationCause::Blur, rule)
    }

    /// Runs on submit.
    pub fn on_submit(self, rule: impl Fn(&H) -> FormValidationResult + 'static) -> Self {
        self.push_sync(ValidationCause::Submit, rule)
    }

    /// Runs on revalidation, see [`ValidationLogic::Revalidate`].
    pub fn on_dynamic(self, rule: impl Fn(&H) -> FormValidationResult + 'static) -> Self {
        self.push_sync(ValidationCause::Dynamic, rule)
    }

    /// An async check on change of any field.
    pub fn on_change_async<F, Fut>(self, rule: F) -> Self
    where
        F: Fn(H) -> Fut + 'static,
        Fut: Future<Output = FormValidationResult> + 'static,
    {
        self.push_async(ValidationCause::Change, rule)
    }

    /// Waits this long after the last edit before the change check runs.
    pub fn on_change_async_debounce(mut self, delay: Duration) -> Self {
        self.debounce[ValidationCause::Change.index()] = Some(delay);
        self
    }

    /// An async check on blur of any field.
    pub fn on_blur_async<F, Fut>(self, rule: F) -> Self
    where
        F: Fn(H) -> Fut + 'static,
        Fut: Future<Output = FormValidationResult> + 'static,
    {
        self.push_async(ValidationCause::Blur, rule)
    }

    /// Waits this long before the blur check runs.
    pub fn on_blur_async_debounce(mut self, delay: Duration) -> Self {
        self.debounce[ValidationCause::Blur.index()] = Some(delay);
        self
    }

    /// An async check on submit. A server's answer goes here: its field messages
    /// show on their fields.
    pub fn on_submit_async<F, Fut>(self, rule: F) -> Self
    where
        F: Fn(H) -> Fut + 'static,
        Fut: Future<Output = FormValidationResult> + 'static,
    {
        self.push_async(ValidationCause::Submit, rule)
    }

    /// An async check on revalidation.
    pub fn on_dynamic_async<F, Fut>(self, rule: F) -> Self
    where
        F: Fn(H) -> Fut + 'static,
        Fut: Future<Output = FormValidationResult> + 'static,
    {
        self.push_async(ValidationCause::Dynamic, rule)
    }

    /// Waits this long before the revalidation check runs.
    pub fn on_dynamic_async_debounce(mut self, delay: Duration) -> Self {
        self.debounce[ValidationCause::Dynamic.index()] = Some(delay);
        self
    }
}

/// Loads the saved values from the persist key, if any.
pub(crate) type Loader<H> = Rc<dyn Fn() -> Option<H>>;

/// Settings and state a form holds for its whole life.
pub(crate) struct FormRuntime<H: 'static> {
    defaults: H,
    persist: Option<Callback<H>>,
    restore: Option<Loader<H>>,
    persist_debounce: Duration,
    persist_slot: Slot,
    last_saved: Option<H>,
    persist_writes: usize,
    form_id: String,
    default_meta: Option<Rc<dyn Any>>,
    validators: FormValidators<H>,
    listeners: FormListeners<H>,
    logic: ValidationLogic,
    async_always: bool,
    async_debounce: Duration,
    can_submit_when_invalid: bool,
    on_submit_invalid: Option<Callback<H>>,
    on_submit: Option<SubmitFn<H>>,
    errors: ErrorMap,
    slots: [Slot; CAUSE_COUNT],
    pending: [bool; CAUSE_COUNT],
    listener_slot: Slot,
}

impl<H: 'static> FormRuntime<H> {
    /// Replaces the settings, keeping the running validators' slots and the current errors.
    fn take_settings(&mut self, new: FormRuntime<H>) {
        self.defaults = new.defaults;
        self.form_id = new.form_id;
        self.default_meta = new.default_meta;
        self.validators = new.validators;
        self.listeners = new.listeners;
        self.logic = new.logic;
        self.async_always = new.async_always;
        self.async_debounce = new.async_debounce;
        self.can_submit_when_invalid = new.can_submit_when_invalid;
        self.on_submit_invalid = new.on_submit_invalid;
        self.on_submit = new.on_submit;
    }
}

/// Everything a form can be made with. Pass it to [`use_form_with`].
pub struct FormOptions<H> {
    default_values: H,
    persist_hook: Option<Callback<H>>,
    restore_hook: Option<Loader<H>>,
    persist_debounce: Duration,
    default_state: Option<FormStateSnapshot<H>>,
    transform: Option<StateTransform<H>>,
    form_id: String,
    default_meta: Option<Rc<dyn Any>>,
    async_always: bool,
    async_debounce: Duration,
    can_submit_when_invalid: bool,
    validators: FormValidators<H>,
    validation_logic: ValidationLogic,
    listeners: FormListeners<H>,
    on_submit: Option<SubmitFn<H>>,
    on_submit_invalid: Option<Callback<H>>,
}

impl<H: 'static> FormOptions<H> {
    /// Options with `default_values` as the baseline, and nothing else set.
    pub fn new(default_values: H) -> Self {
        Self {
            default_values,
            persist_hook: None,
            restore_hook: None,
            persist_debounce: Duration::from_millis(300),
            default_state: None,
            transform: None,
            form_id: String::from("form"),
            default_meta: None,
            async_always: false,
            async_debounce: Duration::ZERO,
            can_submit_when_invalid: false,
            validators: FormValidators::default(),
            validation_logic: ValidationLogic::Default,
            listeners: FormListeners::default(),
            on_submit: None,
            on_submit_invalid: None,
        }
    }

    /// The state to start from: values, errors, field metadata and submission state, as
    /// [`Form::save_state`] returns them. Its values replace `default_values` as the starting values;
    /// `default_values` stays the baseline for dirty tracking.
    pub fn default_state(mut self, state: FormStateSnapshot<H>) -> Self {
        self.default_state = Some(state);
        self
    }

    /// Changes the initial state before the form uses it, as TanStack's `transform` does.
    pub fn transform(
        mut self,
        transform: impl Fn(FormStateSnapshot<H>) -> FormStateSnapshot<H> + 'static,
    ) -> Self {
        self.transform = Some(Rc::new(transform));
        self
    }

    /// Saves the values to NSH's `Storage` under `key` after each change, and starts from the
    /// saved values when one exists. Needs the `persist` feature.
    #[cfg(feature = "persist")]
    pub fn persist(mut self, key: impl Into<String>) -> Self
    where
        H: Clone + serde::Serialize + serde::de::DeserializeOwned,
    {
        let key = key.into();
        let saved = nsh_core::storage::Storage::get_item(&key)
            .and_then(|saved| serde_json::from_str::<H>(&saved).ok());
        if let Some(values) = saved {
            self.default_state = Some(FormStateSnapshot::from_values(values));
        }
        let load_key = key.clone();
        self.restore_hook = Some(Rc::new(move || {
            nsh_core::storage::Storage::get_item(&load_key)
                .and_then(|saved| serde_json::from_str::<H>(&saved).ok())
        }));
        self.persist_hook = Some(Rc::new(move |values: &H| {
            if let Ok(text) = serde_json::to_string(values) {
                let _ = nsh_core::storage::Storage::set_item(key.clone(), text);
            }
        }));
        self
    }

    /// Waits this long after the last change before the persist key is written, so a burst of
    /// edits writes once. Defaults to 300 ms. Changes still waiting when the form's owner goes
    /// are dropped; call [`Form::save_values`] first to keep them.
    pub fn persist_debounce(mut self, delay: Duration) -> Self {
        self.persist_debounce = delay;
        self
    }

    /// The form's identifier, read back with [`Form::form_id`]. Defaults to `"form"`.
    pub fn form_id(mut self, id: impl Into<String>) -> Self {
        self.form_id = id.into();
        self
    }

    /// The meta that [`Form::handle_submit_meta`] passes to its save.
    pub fn on_submit_meta<M: Clone + 'static>(mut self, meta: M) -> Self {
        self.default_meta = Some(Rc::new(meta));
        self
    }

    /// Runs async validators even when a sync validator failed.
    pub fn async_always(mut self, yes: bool) -> Self {
        self.async_always = yes;
        self
    }

    /// The debounce of async validators that do not set their own.
    pub fn async_debounce(mut self, delay: Duration) -> Self {
        self.async_debounce = delay;
        self
    }

    /// Lets `can_submit` stay true while the form is invalid.
    pub fn can_submit_when_invalid(mut self, yes: bool) -> Self {
        self.can_submit_when_invalid = yes;
        self
    }

    /// The form's validators.
    pub fn validators(mut self, validators: FormValidators<H>) -> Self {
        self.validators = validators;
        self
    }

    /// Which validators run for each event.
    pub fn validation_logic(mut self, logic: ValidationLogic) -> Self {
        self.validation_logic = logic;
        self
    }

    /// The form's listeners.
    pub fn listeners(mut self, listeners: FormListeners<H>) -> Self {
        self.listeners = listeners;
        self
    }

    /// The save that [`Form::submit_with_options`] runs once validation passed.
    pub fn on_submit<F, Fut>(mut self, save: F) -> Self
    where
        F: Fn(H) -> Fut + 'static,
        Fut: Future<Output = SubmitResult> + 'static,
    {
        self.on_submit = Some(Rc::new(move |values: H| boxed(save(values))));
        self
    }

    /// Runs when a submit is refused, with the values.
    pub fn on_submit_invalid(mut self, hook: impl Fn(&H) + 'static) -> Self {
        self.on_submit_invalid = Some(Rc::new(hook));
        self
    }

    /// Splits the options into the initial state (after `transform`) and the settings.
    fn into_parts(self) -> (FormStateSnapshot<H>, FormRuntime<H>)
    where
        H: Clone,
    {
        let initial = match self.default_state {
            Some(saved) => saved,
            None => FormStateSnapshot::from_values(self.default_values.clone()),
        };
        let initial = match &self.transform {
            Some(transform) => transform(initial),
            None => initial,
        };
        // The starting values count as saved, so opening the form writes nothing.
        let last_saved = self.persist_hook.as_ref().map(|_| initial.values.clone());
        let runtime = FormRuntime {
            defaults: self.default_values,
            persist: self.persist_hook,
            restore: self.restore_hook,
            persist_debounce: self.persist_debounce,
            persist_slot: Slot::default(),
            last_saved,
            persist_writes: 0,
            form_id: self.form_id,
            default_meta: self.default_meta,
            validators: self.validators,
            listeners: self.listeners,
            logic: self.validation_logic,
            async_always: self.async_always,
            async_debounce: self.async_debounce,
            can_submit_when_invalid: self.can_submit_when_invalid,
            on_submit_invalid: self.on_submit_invalid,
            on_submit: self.on_submit,
            errors: ErrorMap::default(),
            slots: Default::default(),
            pending: [false; CAUSE_COUNT],
            listener_slot: Slot::default(),
        };
        (initial, runtime)
    }
}

/// What [`Form::diagnostics`] reports.
#[derive(Clone, Debug, PartialEq)]
pub struct FormDiagnostics {
    /// Every registered field's path, in registration order.
    pub field_paths: Vec<FieldPath>,
    /// The fields whose value differs from the baseline.
    pub changed_paths: Vec<FieldPath>,
    /// The number of messages across the form and its fields.
    pub error_count: usize,
    /// The async validations running or waiting for a debounce.
    pub pending_validations: usize,
    /// A submit is running.
    pub is_submitting: bool,
    /// The number of submits since the last reset.
    pub submission_attempts: u32,
    /// The form can be submitted now.
    pub can_submit: bool,
    /// Writes made to the persist key since the form was made.
    pub persist_writes: usize,
}

/// A form over the value type `H`. `Copy`. Reads are tracked. Owned by the
/// component that made it.
pub struct Form<H: 'static> {
    pub(crate) state: Stored<FormState<H>>,
    pub(crate) owner: Owner,
    pub(crate) entries: Stored<Entries>,
    fields: Stored<Vec<Rc<dyn AnyField<H>>>>,
    runtime: Stored<FormRuntime<H>>,
    last_root: Stored<Option<H>>,
    pub(crate) flags: FormSignals,
    pub(crate) submitting: Signal<bool>,
    /// Errors show from here on. A submit or a validate sets it.
    pub(crate) submitted: Signal<bool>,
    attempts: Signal<u32>,
    succeeded: Signal<bool>,
    successful: Signal<bool>,
}

impl<H> Clone for Form<H> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<H> Copy for Form<H> {}

/// The form-level flags, each a signal that is set only when its value changes.
#[derive(Clone, Copy)]
pub(crate) struct FormSignals {
    pub root: Signal<u64>,
    pub is_dirty: Signal<bool>,
    pub touched: Signal<bool>,
    pub blurred: Signal<bool>,
    pub changed: Signal<bool>,
    pub fields_valid: Signal<bool>,
    pub fields_validating: Signal<bool>,
    pub form_valid: Signal<bool>,
    pub can_submit: Signal<bool>,
    pub errors: Signal<ErrorMap>,
    pub form_pending: Signal<bool>,
    pub last_error: Signal<Option<SubmitError>>,
}

impl FormSignals {
    fn new() -> Self {
        Self {
            root: signal(0),
            is_dirty: signal(false),
            touched: signal(false),
            blurred: signal(false),
            changed: signal(false),
            fields_valid: signal(true),
            fields_validating: signal(false),
            form_valid: signal(true),
            can_submit: signal(true),
            errors: signal(ErrorMap::default()),
            form_pending: signal(false),
            last_error: signal(None),
        }
    }
}

/// A form from `initial`, which is also the baseline for dirty tracking.
pub fn use_form<H: Clone + PartialEq + 'static>(initial: H) -> Form<H> {
    use_form_with(FormOptions::new(initial))
}

/// A form made from `options`. Its form-level mount validators run now.
pub fn use_form_with<H: Clone + PartialEq + 'static>(options: FormOptions<H>) -> Form<H> {
    let (initial, runtime) = options.into_parts();
    let mut state = FormState::new(runtime.defaults.clone());
    state.replace_current(initial.values.clone());
    let form = Form {
        state: stored(state),
        owner: Owner::current(),
        entries: stored(Entries::new()),
        fields: stored(Vec::new()),
        runtime: stored(runtime),
        last_root: stored(None),
        flags: FormSignals::new(),
        submitting: signal(false),
        submitted: signal(false),
        attempts: signal(0),
        succeeded: signal(false),
        successful: signal(false),
    };
    form.restore_extras(&initial);
    form.mount();
    form
}

/// Shared options. TanStack's `formOptions` only helps type inference; here the options are
/// returned as they are, so a shared set reads the same at each use.
pub fn form_options<H>(options: FormOptions<H>) -> FormOptions<H> {
    options
}

/// A form's whole state, as [`Form::save_state`] returns it and [`FormOptions::default_state`] takes it.
#[derive(Clone, Debug, PartialEq)]
pub struct FormStateSnapshot<H> {
    /// The values.
    pub values: H,
    /// The form-level errors.
    pub error_map: ErrorMap,
    /// Each registered field's metadata, by path.
    pub field_meta: Vec<(FieldPath, FieldMeta)>,
    /// The number of submits.
    pub submission_attempts: u32,
    /// A submit succeeded.
    pub is_submitted: bool,
    /// The last submit succeeded.
    pub is_submit_successful: bool,
}

impl<H> FormStateSnapshot<H> {
    /// A state with these values and nothing else set.
    pub fn from_values(values: H) -> Self {
        Self {
            values,
            error_map: ErrorMap::default(),
            field_meta: Vec::new(),
            submission_attempts: 0,
            is_submitted: false,
            is_submit_successful: false,
        }
    }
}

/// A function that changes the initial state. See [`FormOptions::transform`].
pub(crate) type StateTransform<H> = Rc<dyn Fn(FormStateSnapshot<H>) -> FormStateSnapshot<H>>;

/// Everything a form knows about its state, for [`Form::select`].
#[derive(Clone)]
pub struct FormSnapshot<H> {
    /// The values.
    pub values: H,
    /// The values differ from the baseline.
    pub is_dirty: bool,
    /// A field was edited since the last reset.
    pub was_changed: bool,
    /// The values equal the baseline.
    pub is_default_value: bool,
    /// No field has an error, the form has none, and nothing validates.
    pub is_valid: bool,
    /// See [`Form::can_submit`].
    pub can_submit: bool,
    /// A submit is running.
    pub is_submitting: bool,
    /// A submit succeeded since the last reset.
    pub is_submitted: bool,
    /// The last submit succeeded.
    pub is_submit_successful: bool,
    /// The number of submits since the last reset.
    pub submission_attempts: u32,
    /// A field's async validator is running.
    pub is_validating: bool,
    /// A field was touched.
    pub is_touched: bool,
    /// A field was blurred.
    pub is_blurred: bool,
    /// The form-level errors are empty.
    pub is_form_valid: bool,
    /// No field has an error.
    pub is_fields_valid: bool,
    /// The distinct form-level messages.
    pub errors: Vec<ValidationError>,
}

/// The errors of a form and of each of its fields, for [`Form::get_all_errors`].
#[derive(Clone, Debug, Default)]
pub struct AllErrors {
    /// The form-level errors.
    pub form: ErrorMap,
    /// Each field's errors, by path.
    pub fields: Vec<(FieldPath, ErrorMap)>,
}

/// What a sync pass produced: whether the form failed, and which targets failed.
struct SyncReport {
    form_error: bool,
    fields: Vec<bool>,
}

fn is_within(path: &FieldPath, prefix: &FieldPath) -> bool {
    path.segments().starts_with(prefix.segments())
}

fn default_meta() -> FieldMeta {
    FieldMeta {
        is_touched: false,
        is_blurred: false,
        was_changed: false,
        is_default_value: true,
        is_validating: false,
        error_map: ErrorMap::default(),
    }
}

fn focus_first_invalid<H: 'static>(targets: &[Rc<dyn AnyField<H>>]) {
    if let Some(first) = targets.iter().find(|field| field.invalid()) {
        first.focus();
    }
}

impl<H: Clone + PartialEq + 'static> Form<H> {
    /// Registers a field. Prefer the [`crate::field!`] macro, which writes this call.
    pub fn field<T, G, M>(&self, path: &[&'static str], get: G, get_mut: M) -> Field<H, T>
    where
        T: Clone + PartialEq + 'static,
        G: Fn(&H) -> &T + 'static,
        M: Fn(&mut H) -> &mut T + 'static,
    {
        let path = FieldPath::new(path);
        let locate: Locate = Rc::new(move || Some(path.clone()));
        self.register(access(get, get_mut), locate, None)
    }

    pub(crate) fn register<T: Clone + PartialEq + 'static>(
        &self,
        access: Access<H, T>,
        locate: Locate,
        shape: Option<fn(&T) -> usize>,
    ) -> Field<H, T> {
        // Made under the form's owner, so the registry never holds a field whose owner ended.
        let field = self.owner.run(|| Field::new(*self, access, locate, shape));
        self.fields.with(|fields| fields.push(Rc::new(field)));
        self.bump();
        field
    }

    /// Brings every binding of the form up to date after a change. Each signal is set
    /// only if its value changed, so a binding wakes only for what it reads.
    pub(crate) fn bump(&self) {
        let now = self.values_untracked();
        let differs = self.last_root.with(|last| last.as_ref() != Some(&now));
        if differs {
            // The write waits for the debounce, so a burst of changes writes once.
            self.schedule_save();
            self.last_root.with(|last| *last = Some(now));
        }
        batch(|| {
            for field in self.registered() {
                field.publish();
            }
            let nodes: Vec<(Signals, Entry)> = self.entries.with(|entries| {
                entries
                    .values()
                    .map(|node| (node.sig, node.entry.clone()))
                    .collect()
            });
            for (sig, entry) in &nodes {
                sig.apply(entry);
            }
            if differs {
                self.flags.root.update(|root| *root = root.wrapping_add(1));
            }
            self.refresh_flags();
        });
    }

    /// Starts the debounced save, restarting it so the last change sets the write time.
    fn schedule_save(&self) {
        let (persisting, delay) = self
            .runtime
            .with(|runtime| (runtime.persist.is_some(), runtime.persist_debounce));
        if !persisting {
            return;
        }
        let generation = self.runtime.with(|runtime| runtime.persist_slot.restart());
        let form = *self;
        let task = spawn(async move {
            timer(delay).await;
            if form
                .runtime
                .with(|runtime| runtime.persist_slot.is_current(generation))
            {
                form.write_saved();
            }
        });
        self.runtime
            .with(|runtime| runtime.persist_slot.set_task(task));
    }

    /// Writes the current values to the persist key, if they differ from the last saved copy.
    fn write_saved(&self) {
        let now = self.values_untracked();
        let persist = self.runtime.with(|runtime| runtime.persist.clone());
        let Some(persist) = persist else {
            return;
        };
        let differs = self
            .runtime
            .with(|runtime| runtime.last_saved.as_ref() != Some(&now));
        if !differs {
            return;
        }
        persist(&now);
        self.runtime.with(|runtime| {
            runtime.last_saved = Some(now);
            runtime.persist_writes += 1;
        });
    }

    /// Writes the current values to the persist key now, and drops the save waiting for its
    /// debounce. Does nothing without `FormOptions::persist`.
    pub fn save_values(&self) {
        self.runtime.with(|runtime| runtime.persist_slot.cancel());
        self.write_saved();
    }

    /// Loads the saved values as the new values and baseline. Any save still waiting for its
    /// debounce is dropped, so unsaved edits are lost and a stale save cannot overwrite the
    /// loaded values. Does nothing when nothing is saved.
    pub fn restore_values(&self) {
        self.runtime.with(|runtime| runtime.persist_slot.cancel());
        let loader = self.runtime.with(|runtime| runtime.restore.clone());
        let Some(values) = loader.and_then(|load| load()) else {
            return;
        };
        self.runtime
            .with(|runtime| runtime.last_saved = Some(values.clone()));
        self.reset_with(values);
    }

    /// Puts the errors, field metadata and submission state of `state` in place. Values are
    /// set by the caller.
    fn restore_extras(&self, state: &FormStateSnapshot<H>) {
        self.runtime
            .with(|runtime| runtime.errors = state.error_map.clone());
        self.attempts.set(state.submission_attempts);
        self.succeeded.set(state.is_submitted);
        self.successful.set(state.is_submit_successful);
        for (path, meta) in &state.field_meta {
            self.signals_for(path);
            self.entries.with(|entries| {
                if let Some(node) = entries.get_mut(path) {
                    node.entry.touched = meta.is_touched;
                    node.entry.blurred = meta.is_blurred;
                    node.entry.dirty = meta.was_changed;
                    node.entry.own = meta.error_map.clone();
                }
            });
        }
        self.bump();
    }

    /// The whole state: values, form-level errors, every registered field's metadata and the
    /// submission state. Pass it to [`FormOptions::default_state`] to restore it later. Tracked.
    pub fn save_state(&self) -> FormStateSnapshot<H> {
        let field_meta = self
            .registered()
            .into_iter()
            .filter_map(|field| field.path().map(|path| (path, field.snapshot())))
            .collect();
        FormStateSnapshot {
            values: self.values(),
            error_map: self.error_map(),
            field_meta,
            submission_attempts: self.attempts.get(),
            is_submitted: self.succeeded.get(),
            is_submit_successful: self.successful.get(),
        }
    }

    /// Deep-merges a state into the form, as TanStack's `mergeForm` does. A typed form cannot merge
    /// field by field, so the values are replaced as a whole. Errors, metadata and submission state
    /// are set from `state`.
    pub fn merge_form(&self, state: FormStateSnapshot<H>) {
        self.state
            .with(|current| current.replace_current(state.values.clone()));
        self.restore_extras(&state);
    }

    /// Replaces the options at run time, as TanStack's `form.update`. When the default values
    /// change and the form is untouched, the values and baseline move to them. A `default_state`
    /// in the new options is restored in the same case.
    pub fn update(&self, options: FormOptions<H>) {
        let untouched = !self.flags.touched.get_untracked();
        let state = options.default_state.clone();
        let (initial, settings) = options.into_parts();
        if untouched
            && self
                .runtime
                .with(|runtime| runtime.defaults != settings.defaults)
        {
            let defaults = settings.defaults.clone();
            self.state
                .with(|current| *current = FormState::new(defaults));
        }
        self.runtime.with(|runtime| runtime.take_settings(settings));
        if untouched && state.is_some() {
            self.state
                .with(|current| current.replace_current(initial.values.clone()));
            self.restore_extras(&initial);
        }
        self.bump();
    }

    /// Recomputes the form-level flags from the entries. Each signal is set only if its value changed.
    fn refresh_flags(&self) {
        let mut touched = false;
        let mut blurred = false;
        let mut changed = false;
        let mut mount_error = false;
        let mut fields_valid = true;
        let mut fields_validating = false;
        for field in self.registered() {
            let entry = field.entry();
            let effective = entry.effective();
            touched |= entry.touched;
            blurred |= entry.blurred;
            changed |= entry.dirty;
            fields_valid &= effective.is_empty();
            fields_validating |= entry.is_validating();
            mount_error |= effective.get(ValidationCause::Mount).is_some();
        }
        let errors = self.runtime.with(|runtime| runtime.errors.clone());
        let form_valid = errors.is_empty();
        mount_error |= errors.get(ValidationCause::Mount).is_some();
        let attempts = self.attempts.get_untracked();
        let submitting = self.submitting.get_untracked();
        let when_invalid = self.runtime.with(|runtime| runtime.can_submit_when_invalid);
        let valid = fields_valid && form_valid;
        let can_submit = (attempts == 0 && !touched && !mount_error)
            || (!fields_validating && !submitting && valid)
            || when_invalid;
        let form_pending = self
            .runtime
            .with(|runtime| runtime.pending.iter().any(|pending| *pending));
        let is_dirty = self.state.with(|state| state.is_dirty());
        let flags = &self.flags;
        flags.is_dirty.set(is_dirty);
        flags.touched.set(touched);
        flags.blurred.set(blurred);
        flags.changed.set(changed);
        flags.fields_valid.set(fields_valid);
        flags.fields_validating.set(fields_validating);
        flags.form_valid.set(form_valid);
        flags.can_submit.set(can_submit);
        flags.errors.set(errors);
        flags.form_pending.set(form_pending);
    }

    /// The signals of the path, made under the form's owner if they do not exist yet.
    pub(crate) fn signals_for(&self, path: &FieldPath) -> Signals {
        if let Some(sig) = self
            .entries
            .with(|entries| entries.get(path).map(|node| node.sig))
        {
            return sig;
        }
        let sig = self.owner.run(Signals::new);
        self.entries.with(|entries| {
            entries
                .entry(path.clone())
                .or_insert_with(|| Node {
                    entry: Entry::default(),
                    sig,
                })
                .sig
        })
    }

    pub(crate) fn registered(&self) -> Vec<Rc<dyn AnyField<H>>> {
        self.fields.with(|fields| fields.clone())
    }

    fn find(&self, path: &FieldPath) -> Option<Rc<dyn AnyField<H>>> {
        self.registered()
            .into_iter()
            .find(|field| field.path().as_ref() == Some(path))
    }

    pub(crate) fn async_debounce_default(&self) -> Duration {
        self.runtime.with(|runtime| runtime.async_debounce)
    }

    fn runs_async_always(&self) -> bool {
        self.runtime.with(|runtime| runtime.async_always)
    }

    fn causes(&self, event: Event) -> Vec<ValidationCause> {
        let logic = self.runtime.with(|runtime| runtime.logic);
        plan(event, logic, self.attempts.get_untracked())
    }

    /// The values. Tracked: it wakes when a value changes.
    pub fn values(&self) -> H {
        self.flags.root.get();
        self.values_untracked()
    }

    /// The values, without tracking.
    pub fn values_untracked(&self) -> H {
        self.state.with(|state| state.current().clone())
    }

    /// Replaces every value at once and clears errors and touched flags. The baseline stays.
    pub fn set_values(&self, values: H) {
        self.state.with(|state| state.replace_current(values));
        self.clear_feedback();
        self.bump();
    }

    /// Whether the values differ from the baseline. Kept for existing callers;
    /// [`Form::was_changed`] is TanStack's persistent flag. Tracked.
    pub fn is_dirty(&self) -> bool {
        self.flags.is_dirty.get()
    }

    /// Whether the values equal the baseline. Tracked. The inverse of [`Form::is_dirty`].
    pub fn is_default_value(&self) -> bool {
        !self.flags.is_dirty.get()
    }

    /// Whether a field was edited since the last reset. Stays true after the value
    /// returns to its default. Tracked.
    pub fn was_changed(&self) -> bool {
        self.flags.changed.get()
    }

    /// Whether no field was edited since the last reset. Tracked.
    pub fn is_pristine(&self) -> bool {
        !self.was_changed()
    }

    /// Whether a field was touched since the last reset. Tracked.
    pub fn is_touched(&self) -> bool {
        self.flags.touched.get()
    }

    /// Whether a field was blurred since the last reset. Tracked.
    pub fn is_blurred(&self) -> bool {
        self.flags.blurred.get()
    }

    /// Whether no field has an error, the form has none, and no field is validating.
    /// Tracked. Kept as it was for existing callers.
    pub fn is_valid(&self) -> bool {
        self.flags.fields_valid.get()
            && self.flags.form_valid.get()
            && !self.flags.fields_validating.get()
    }

    /// Whether no field has an error. Tracked.
    pub fn is_fields_valid(&self) -> bool {
        self.flags.fields_valid.get()
    }

    /// Whether the form-level errors are empty. Tracked.
    pub fn is_form_valid(&self) -> bool {
        self.flags.form_valid.get()
    }

    /// Whether a field's async validator is running. Tracked. TanStack's `isValidating`.
    pub fn is_validating(&self) -> bool {
        self.flags.fields_validating.get()
    }

    /// Whether a field's async validator is running. Tracked. TanStack's `isFieldsValidating`.
    pub fn is_fields_validating(&self) -> bool {
        self.flags.fields_validating.get()
    }

    /// Whether a form-level async validator is running. Tracked.
    pub fn is_form_validating(&self) -> bool {
        self.flags.form_pending.get()
    }

    /// Whether the form can be submitted, as TanStack's `canSubmit`. A form that was
    /// never submitted, touched or mount-failed can submit. Otherwise it needs to be
    /// valid, not validating and not submitting. `can_submit_when_invalid` overrides the rest. Tracked.
    pub fn can_submit(&self) -> bool {
        self.flags.can_submit.get()
    }

    /// Whether a submit is running. Tracked.
    pub fn is_submitting(&self) -> bool {
        self.submitting.get()
    }

    /// Whether a submit succeeded since the last reset. Tracked. TanStack's `isSubmitted`.
    pub fn is_submitted(&self) -> bool {
        self.succeeded.get()
    }

    /// Whether the last submit succeeded. Tracked. TanStack's `isSubmitSuccessful`.
    pub fn is_submit_successful(&self) -> bool {
        self.successful.get()
    }

    /// The number of submits since the last reset. Tracked.
    pub fn submission_attempts(&self) -> u32 {
        self.attempts.get()
    }

    /// The number of submits since the last reset. Tracked. Same as [`Form::submission_attempts`].
    pub fn submit_count(&self) -> u32 {
        self.submission_attempts()
    }

    /// The form-level errors, keyed by cause. Tracked.
    pub fn error_map(&self) -> ErrorMap {
        self.flags.errors.get()
    }

    /// The distinct form-level error messages. Tracked.
    pub fn errors(&self) -> Vec<ValidationError> {
        self.error_map().messages()
    }

    /// The first form-level message, from the last submit or validator. Tracked.
    pub fn error(&self) -> Option<String> {
        self.error_map().first()
    }

    /// The registration record of the field at `path`, as TanStack's `getFieldInfo`. `None` when no
    /// field is registered there.
    pub fn get_field_info(&self, path: &FieldPath) -> Option<FieldInfo> {
        self.find(path).map(|field| FieldInfo {
            path: path.clone(),
            registered: true,
            validation: field.validation(),
        })
    }

    /// The metadata before the derived values, as TanStack's `fieldMetaBase`, for the field at `path`.
    pub fn field_meta_base(&self, path: &FieldPath) -> Option<FieldMetaBase> {
        self.find(path).map(|field| field.meta_base())
    }

    /// The metadata of the field at `path`, if it is registered. Tracked.
    pub fn field_meta(&self, path: &FieldPath) -> Option<FieldMeta> {
        self.find(path).map(|field| field.snapshot())
    }

    /// Everything the form knows about its state, as a snapshot. Tracked.
    pub fn snapshot(&self) -> FormSnapshot<H> {
        FormSnapshot {
            values: self.values(),
            is_dirty: self.is_dirty(),
            was_changed: self.was_changed(),
            is_default_value: self.is_default_value(),
            is_valid: self.is_valid(),
            can_submit: self.can_submit(),
            is_submitting: self.submitting.get(),
            is_submitted: self.succeeded.get(),
            is_submit_successful: self.successful.get(),
            submission_attempts: self.attempts.get(),
            is_validating: self.is_validating(),
            is_touched: self.is_touched(),
            is_blurred: self.is_blurred(),
            is_form_valid: self.is_form_valid(),
            is_fields_valid: self.is_fields_valid(),
            errors: self.errors(),
        }
    }

    /// A memo of part of the form state. A binding that reads it reruns only when that part changes.
    pub fn select<R: PartialEq + 'static>(
        &self,
        pick: impl Fn(&FormSnapshot<H>) -> R + 'static,
    ) -> Memo<R> {
        let form = *self;
        computed(move || pick(&form.snapshot()))
    }

    /// Every error of the form and of its fields.
    pub fn get_all_errors(&self) -> AllErrors {
        let fields = self
            .registered()
            .into_iter()
            .filter_map(|field| field.path().map(|path| (path, field.snapshot().error_map)))
            .collect();
        AllErrors {
            form: self.error_map(),
            fields,
        }
    }

    /// Back to the baseline, with errors and touched flags cleared, and the submit count reset.
    pub fn reset(&self) {
        self.state.with(FormState::reset_to_baseline);
        self.clear_feedback();
        self.attempts.set(0);
        self.bump();
    }

    /// Makes the current values the baseline, so the form reads clean. A successful submit does this too.
    pub fn mark_saved(&self) {
        self.state.with(FormState::sync_baseline);
        self.bump();
    }

    /// Replaces values and baseline, for data that loaded after the form was made.
    pub fn load(&self, values: H) {
        self.reset_with(values);
    }

    /// Same as [`Form::reset`], with `values` as the new baseline.
    pub fn reset_with(&self, values: H) {
        self.state.with(|state| *state = FormState::new(values));
        self.clear_feedback();
        self.attempts.set(0);
        self.bump();
    }

    /// Restores one field to its baseline value, and clears its metadata.
    pub fn reset_field(&self, path: &FieldPath) {
        match self.find(path) {
            Some(field) => field.reset_value(),
            None => {
                self.entries.with(|entries| {
                    if let Some(node) = entries.get_mut(path) {
                        node.entry = Entry::default();
                    }
                });
                self.bump();
            }
        }
    }

    /// Clears one field's metadata, errors and pending validators. Its value stays.
    pub fn reset_field_meta(&self, path: &FieldPath) {
        if let Some(field) = self.find(path) {
            field.cancel_all();
        }
        self.entries.with(|entries| {
            if let Some(node) = entries.get_mut(path) {
                node.entry = Entry::default();
            }
        });
        self.bump();
    }

    /// Changes the metadata of the field at `path`. The touched, blurred and changed
    /// flags and the error map are written back.
    pub fn set_field_meta(&self, path: &FieldPath, change: impl FnOnce(&mut FieldMeta)) {
        let mut meta = self
            .find(path)
            .map(|field| field.snapshot())
            .unwrap_or_else(default_meta);
        change(&mut meta);
        self.signals_for(path);
        self.entries.with(|entries| {
            if let Some(node) = entries.get_mut(path) {
                node.entry.touched = meta.is_touched;
                node.entry.blurred = meta.is_blurred;
                node.entry.dirty = meta.was_changed;
                node.entry.own = meta.error_map;
            }
        });
        self.bump();
    }

    /// Removes the registration of the field at `path` and of the fields under it, and
    /// clears their metadata and errors. The values stay, since a typed field cannot be
    /// absent. [`Field::delete_value`] puts the default of the type in place.
    pub fn delete_field(&self, path: &FieldPath) {
        let doomed: Vec<Rc<dyn AnyField<H>>> = self
            .registered()
            .into_iter()
            .filter(|field| field.path().is_some_and(|own| is_within(&own, path)))
            .collect();
        for field in &doomed {
            field.cancel_all();
        }
        self.fields.with(|fields| {
            fields.retain(|field| !doomed.iter().any(|gone| gone.key() == field.key()));
        });
        self.entries.with(|entries| {
            for (own, node) in entries.iter_mut() {
                if is_within(own, path) {
                    node.entry = Entry::default();
                }
            }
        });
        self.bump();
    }

    /// Replaces the form-level errors.
    pub fn set_error_map(&self, map: ErrorMap) {
        self.runtime.with(|runtime| runtime.errors = map);
        self.bump();
    }

    /// Runs when a submit is refused, with the values.
    pub fn on_submit_invalid(&self, hook: impl Fn(&H) + 'static) {
        self.runtime
            .with(|runtime| runtime.on_submit_invalid = Some(Rc::new(hook)));
    }

    /// Lets a submit go ahead while the form is invalid, so `can_submit` stays true.
    pub fn can_submit_when_invalid(&self, yes: bool) {
        self.runtime
            .with(|runtime| runtime.can_submit_when_invalid = yes);
        self.bump();
    }

    /// The debounce of async validators that do not set their own.
    pub fn async_debounce(&self, delay: Duration) {
        self.runtime.with(|runtime| runtime.async_debounce = delay);
    }

    /// Whether async validators run even when a sync validator failed.
    pub fn async_always(&self, yes: bool) {
        self.runtime.with(|runtime| runtime.async_always = yes);
    }

    /// Runs the form's mount validators and listener, and the mount validators of every field.
    pub fn mount(&self) {
        let targets = self.registered();
        self.run_event(Event::Mount, &targets, false);
        let listener = self
            .runtime
            .with(|runtime| runtime.listeners.on_mount.clone());
        if let Some(listener) = listener {
            listener(&self.values_untracked());
        }
    }

    /// Shows every field's error, focuses the first invalid field that has a bound
    /// input, and says whether the form is valid. Runs the sync validators only.
    pub fn validate(&self) -> bool {
        self.submitted.set(true);
        self.mark_all_touched();
        let targets = self.registered();
        self.run_event(Event::Submit, &targets, false);
        if self.any_error_now() {
            focus_first_invalid(&targets);
            false
        } else {
            true
        }
    }

    /// Runs the validators of one cause on every field, and shows the errors.
    pub fn validate_all_fields(&self, cause: ValidationCause) {
        self.submitted.set(true);
        self.mark_all_touched();
        let targets = self.registered();
        self.run_event(Event::of_cause(cause), &targets, true);
    }

    /// Runs the validators of one cause on the field at `path`, and returns its messages.
    pub fn validate_field(&self, path: &FieldPath, cause: ValidationCause) -> Vec<ValidationError> {
        let Some(field) = self.find(path) else {
            return Vec::new();
        };
        field.mark_touched();
        self.run_event(Event::of_cause(cause), std::slice::from_ref(&field), true);
        field.snapshot().errors()
    }

    /// Validates, then runs `save` with the values. A submit that is already running does nothing.
    /// On success the values become the baseline. On failure the errors show on their fields.
    pub fn submit<F, Fut>(&self, save: F)
    where
        F: FnOnce(H) -> Fut + 'static,
        Fut: Future<Output = SubmitResult> + 'static,
    {
        self.submit_flow(Box::new(move |values: H| boxed(save(values))));
    }

    /// Same as [`Form::submit`]. The TanStack name `handleSubmit`.
    pub fn handle_submit<F, Fut>(&self, save: F)
    where
        F: FnOnce(H) -> Fut + 'static,
        Fut: Future<Output = SubmitResult> + 'static,
    {
        self.submit(save);
    }

    /// Like [`Form::submit`], and `save` also gets `meta`, which the caller passes in.
    pub fn handle_submit_with_meta<M, F, Fut>(&self, meta: M, save: F)
    where
        M: 'static,
        F: FnOnce(H, M) -> Fut + 'static,
        Fut: Future<Output = SubmitResult> + 'static,
    {
        self.submit_flow(Box::new(move |values: H| boxed(save(values, meta))));
    }

    /// The error of the last submit, if it failed with one. Cf. TanStack's `SubmitTracker::last_error`.
    pub fn last_error(&self) -> Option<SubmitError> {
        self.flags.last_error.get()
    }

    /// Takes the error of the last submit, leaving none. Cf. `SubmitTracker::take_last_error`.
    pub fn take_last_error(&self) -> Option<SubmitError> {
        let error = self.flags.last_error.get_untracked();
        self.flags.last_error.set(None);
        error
    }

    /// Sets the error a submit reports, as if the save had returned it. Cf. `SubmitTracker::set_error`.
    pub fn set_error(&self, error: SubmitError) {
        let targets = self.registered();
        self.apply_submit_error(error, &targets);
    }

    /// Clears the form-level error and the last submit's error. Cf. `SubmitTracker::clear_error`.
    pub fn clear_error(&self) {
        self.flags.last_error.set(None);
        self.runtime
            .with(|runtime| runtime.errors.set(ValidationCause::Server, None));
        self.bump();
    }

    /// The fields whose value differs from the baseline, in registration order. Tracked. The
    /// changed values are what a patch sends: [`Field::changed_value`] gives each one.
    pub fn changed_paths(&self) -> Vec<FieldPath> {
        self.registered()
            .into_iter()
            .filter(|field| !field.snapshot().is_default_value)
            .filter_map(|field| field.path())
            .collect()
    }

    /// Whether the field at `path` differs from its baseline value. Tracked.
    pub fn is_field_dirty(&self, path: &FieldPath) -> bool {
        self.find(path)
            .is_some_and(|field| !field.snapshot().is_default_value)
    }

    /// A snapshot for a debug panel: the registered paths, changed paths, error and pending counts,
    /// and the submit state. Tracked.
    pub fn diagnostics(&self) -> FormDiagnostics {
        let fields = self.registered();
        let field_paths = fields.iter().filter_map(|field| field.path()).collect();
        let error_count = fields
            .iter()
            .map(|field| field.snapshot().errors().len())
            .sum::<usize>()
            + self.errors().len();
        let pending_validations = fields
            .iter()
            .filter(|field| field.snapshot().is_validating)
            .count()
            + usize::from(self.is_form_validating());
        FormDiagnostics {
            field_paths,
            changed_paths: self.changed_paths(),
            error_count,
            pending_validations,
            is_submitting: self.is_submitting(),
            submission_attempts: self.submission_attempts(),
            can_submit: self.can_submit(),
            persist_writes: self.runtime.with(|runtime| runtime.persist_writes),
        }
    }

    /// Submits with the save given in [`FormOptions::on_submit`]. Does nothing without one.
    pub fn submit_with_options(&self) {
        let save = self.runtime.with(|runtime| runtime.on_submit.clone());
        if let Some(save) = save {
            self.submit_flow(Box::new(move |values: H| save(values)));
        }
    }

    /// Checks the values with `schema` now. Issues show on their fields and the form, and the
    /// values come back only when there are none. Cf. TanStack's `parseValuesWithSchema`.
    pub fn parse_values_with_schema(
        &self,
        schema: &dyn crate::Schema<H>,
    ) -> Result<H, Vec<crate::SchemaIssue>> {
        let values = self.values_untracked();
        let issues = schema.check(&values);
        self.show_issues(&issues);
        if issues.is_empty() {
            Ok(values)
        } else {
            Err(issues)
        }
    }

    /// The async form of [`Form::parse_values_with_schema`].
    pub async fn parse_values_with_schema_async<S: crate::AsyncSchema<H>>(
        &self,
        schema: &S,
    ) -> Result<H, Vec<crate::SchemaIssue>> {
        let values = self.values_untracked();
        let issues = schema.check(values.clone()).await;
        self.show_issues(&issues);
        if issues.is_empty() {
            Ok(values)
        } else {
            Err(issues)
        }
    }

    fn show_issues(&self, issues: &[crate::SchemaIssue]) {
        self.submitted.set(true);
        self.apply_form_result(
            ValidationCause::Submit,
            crate::schema::to_result(issues.to_vec()),
        );
    }

    /// The identifier given with [`FormOptions::form_id`]. Defaults to `"form"`.
    pub fn form_id(&self) -> String {
        self.runtime.with(|runtime| runtime.form_id.clone())
    }

    /// The value of the field at `path`, read as `T`. `None` when no field is registered there, or
    /// when its value is not a `T`. Tracked: it wakes when that value changes.
    pub fn get_field_value<T: 'static>(&self, path: &FieldPath) -> Option<T> {
        self.find(path)?
            .value_any()
            .downcast::<T>()
            .ok()
            .map(|value| *value)
    }

    /// Submits like [`Form::handle_submit_with_meta`], with the meta given to
    /// [`FormOptions::on_submit_meta`]. Without one, the meta is `M::default()`.
    pub fn handle_submit_meta<M, F, Fut>(&self, save: F)
    where
        M: Clone + Default + 'static,
        F: FnOnce(H, M) -> Fut + 'static,
        Fut: Future<Output = SubmitResult> + 'static,
    {
        let meta = self
            .runtime
            .with(|runtime| {
                runtime
                    .default_meta
                    .as_ref()
                    .and_then(|meta| meta.downcast_ref::<M>())
                    .cloned()
            })
            .unwrap_or_default();
        self.submit_flow(Box::new(move |values: H| boxed(save(values, meta))));
    }

    fn submit_flow(&self, save: SaveFn<H>) {
        if self.submitting.get_untracked() {
            return;
        }
        self.mark_all_touched();
        self.attempts
            .update(|count| *count = count.saturating_add(1));
        self.submitted.set(true);
        self.succeeded.set(false);
        self.successful.set(false);
        self.bump();

        let targets = self.registered();
        let values = self.values_untracked();
        let causes = self.causes(Event::Submit);
        let report = self.run_sync(&causes, &targets);
        let always = self.runs_async_always();
        let invalid = self.any_error_now();
        if invalid && !always {
            self.reject(&targets, &values);
            return;
        }
        let has_async = targets
            .iter()
            .any(|field| causes.iter().any(|cause| field.has_async(*cause)))
            || causes.iter().any(|cause| self.has_form_async(*cause));
        let form = *self;
        if !has_async {
            if invalid {
                self.reject(&targets, &values);
                return;
            }
            self.submitting.set(true);
            spawn(async move { form.succeed(values, save, targets).await });
            return;
        }
        self.submitting.set(true);
        let any_failed = report.form_error || report.fields.iter().any(|failed| *failed);
        let failed = report.fields;
        spawn(async move {
            form.run_submit_checks(causes, &targets, failed, any_failed, &values)
                .await;
            if form.any_error_now() {
                form.submitting.set(false);
                form.reject(&targets, &values);
                return;
            }
            form.succeed(values, save, targets).await;
        });
    }

    /// Runs the submit's async checks concurrently, as TanStack's `Promise.all` does. A check
    /// whose field failed is skipped unless `async_always` is set.
    async fn run_submit_checks(
        self,
        causes: Vec<ValidationCause>,
        targets: &[Rc<dyn AnyField<H>>],
        failed: Vec<bool>,
        any_failed: bool,
        values: &H,
    ) {
        let always = self.runs_async_always();
        let mut jobs = Vec::new();
        for (field, failed) in targets.iter().zip(failed) {
            for cause in &causes {
                if !field.has_async(*cause) {
                    continue;
                }
                if failed && !always {
                    field.cancel_async(*cause);
                    continue;
                }
                if let Some(job) = field.job(*cause, values) {
                    let generation = field.begin_async(*cause);
                    let field = Rc::clone(field);
                    let cause = *cause;
                    jobs.push(Box::pin(async move {
                        let answer = job.await;
                        field.finish_async(cause, generation, answer);
                    }) as Pin<Box<dyn Future<Output = ()>>>);
                }
            }
        }
        for &cause in &causes {
            if !self.has_form_async(cause) {
                continue;
            }
            if any_failed && !always {
                self.cancel_form_async(cause);
                continue;
            }
            if let Some(job) = self.form_job(cause, values) {
                let generation = self.begin_form_async(cause);
                let form = self;
                jobs.push(Box::pin(async move {
                    let result = job.await;
                    form.finish_form_async(cause, generation, result);
                }) as Pin<Box<dyn Future<Output = ()>>>);
            }
        }
        let handles: Vec<Spawned<()>> = jobs.into_iter().map(spawn).collect();
        for handle in handles {
            handle.await;
        }
    }

    async fn succeed(self, values: H, save: SaveFn<H>, targets: Vec<Rc<dyn AnyField<H>>>) {
        for field in &targets {
            field.fire_submit();
        }
        let listener = self
            .runtime
            .with(|runtime| runtime.listeners.on_submit.clone());
        if let Some(listener) = listener {
            listener(&values);
        }
        let outcome = save(values.clone()).await;
        match outcome {
            Ok(()) => {
                self.state.with(FormState::sync_baseline);
                self.succeeded.set(true);
                self.successful.set(true);
            }
            Err(error) => {
                self.successful.set(false);
                self.apply_submit_error(error, &targets);
            }
        }
        self.bump();
        self.submitting.set(false);
    }

    fn apply_submit_error(&self, error: SubmitError, targets: &[Rc<dyn AnyField<H>>]) {
        self.flags.last_error.set(Some(error.clone()));
        if let Some(message) = error.form {
            self.runtime
                .with(|runtime| runtime.errors.set(ValidationCause::Server, Some(message)));
        }
        for (path, message) in error.fields {
            let target = targets.iter().find(|field| {
                field.path().map(|own| own.to_string()).as_deref() == Some(path.as_str())
            });
            if let Some(field) = target {
                field.set_server(message);
            }
        }
        self.bump();
    }

    fn reject(&self, targets: &[Rc<dyn AnyField<H>>], values: &H) {
        focus_first_invalid(targets);
        let hook = self
            .runtime
            .with(|runtime| runtime.on_submit_invalid.clone());
        if let Some(hook) = hook {
            hook(values);
        }
    }

    fn any_error_now(&self) -> bool {
        self.registered().iter().any(|field| field.invalid())
            || self.runtime.with(|runtime| !runtime.errors.is_empty())
    }

    fn mark_all_touched(&self) {
        for field in self.registered() {
            field.mark_touched();
        }
        self.bump();
    }

    fn clear_feedback(&self) {
        for field in self.registered() {
            field.cancel_all();
        }
        self.entries.with(|entries| {
            for node in entries.values_mut() {
                node.entry = Entry::default();
            }
        });
        self.runtime.with(|runtime| {
            runtime.errors = ErrorMap::default();
            for slot in runtime.slots.iter_mut() {
                slot.cancel();
            }
            runtime.pending = [false; CAUSE_COUNT];
            runtime.listener_slot.cancel();
        });
        self.submitted.set(false);
        self.succeeded.set(false);
        self.successful.set(false);
    }

    /// Runs the sync validators of `causes`: the form's first, then each target's.
    fn run_sync(&self, causes: &[ValidationCause], targets: &[Rc<dyn AnyField<H>>]) -> SyncReport {
        let values = self.values_untracked();
        let mut form_error = false;
        for cause in causes {
            let rules = self
                .runtime
                .with(|runtime| runtime.validators.sync[cause.index()].clone());
            let mut result = FormValidationResult::default();
            for rule in rules {
                let found = rule(&values);
                if result.form.is_none() {
                    result.form = found.form;
                }
                result.fields.extend(found.fields);
            }
            form_error |= result.form.is_some();
            self.apply_form_result(*cause, result);
        }
        let fields = targets
            .iter()
            .map(|field| field.sync(causes, &values))
            .collect();
        SyncReport { form_error, fields }
    }

    /// Stores a form-level result: the form's error for `cause`, and each field's message.
    fn apply_form_result(&self, cause: ValidationCause, result: FormValidationResult) {
        self.runtime
            .with(|runtime| runtime.errors.set(cause, result.form.clone()));
        for field in self.registered() {
            let message = field
                .path()
                .and_then(|path| result.field_message(&path.to_string()).map(str::to_owned));
            field.set_from_form(cause, message);
        }
        self.bump();
    }

    /// Runs an event: the sync validators of its causes and, when `with_async` is set,
    /// the async ones. Returns whether anything failed.
    pub(crate) fn run_event(
        &self,
        event: Event,
        targets: &[Rc<dyn AnyField<H>>],
        with_async: bool,
    ) -> bool {
        let causes = self.causes(event);
        let report = self.run_sync(&causes, targets);
        let any_failed = report.form_error || report.fields.iter().any(|failed| *failed);
        if with_async {
            let values = self.values_untracked();
            let always = self.runs_async_always();
            for (field, failed) in targets.iter().zip(&report.fields) {
                for cause in &causes {
                    if !field.has_async(*cause) {
                        continue;
                    }
                    if *failed && !always {
                        field.cancel_async(*cause);
                    } else {
                        field.start_async_event(*cause, &values);
                    }
                }
            }
            for cause in &causes {
                if !self.has_form_async(*cause) {
                    continue;
                }
                if any_failed && !always {
                    self.cancel_form_async(*cause);
                } else {
                    self.start_form_async_event(*cause, &values);
                }
            }
        }
        any_failed
    }

    fn has_form_async(&self, cause: ValidationCause) -> bool {
        self.runtime
            .with(|runtime| !runtime.validators.asynch[cause.index()].is_empty())
    }

    fn form_job(&self, cause: ValidationCause, values: &H) -> Option<FormJob> {
        let (rules, delay) = self.runtime.with(|runtime| {
            let rules = runtime.validators.asynch[cause.index()].clone();
            let delay =
                runtime.validators.debounce[cause.index()].unwrap_or(runtime.async_debounce);
            (rules, delay)
        });
        if rules.is_empty() {
            return None;
        }
        let values = values.clone();
        let job: FormJob = Box::pin(async move {
            if !delay.is_zero() {
                timer(delay).await;
            }
            let mut merged = FormValidationResult::default();
            for rule in rules {
                let found = rule(values.clone()).await;
                if merged.form.is_none() {
                    merged.form = found.form;
                }
                merged.fields.extend(found.fields);
            }
            merged
        });
        Some(job)
    }

    fn begin_form_async(&self, cause: ValidationCause) -> u64 {
        let generation = self.runtime.with(|runtime| {
            runtime.pending[cause.index()] = true;
            runtime.slots[cause.index()].restart()
        });
        self.bump();
        generation
    }

    fn finish_form_async(
        &self,
        cause: ValidationCause,
        generation: u64,
        result: FormValidationResult,
    ) {
        let current = self
            .runtime
            .with(|runtime| runtime.slots[cause.index()].is_current(generation));
        if !current {
            return;
        }
        self.apply_form_result(cause, result);
        self.runtime
            .with(|runtime| runtime.pending[cause.index()] = false);
        self.bump();
    }

    fn cancel_form_async(&self, cause: ValidationCause) {
        self.runtime.with(|runtime| {
            runtime.slots[cause.index()].cancel();
            runtime.pending[cause.index()] = false;
        });
        self.bump();
    }

    fn start_form_async_event(&self, cause: ValidationCause, values: &H) {
        let Some(job) = self.form_job(cause, values) else {
            return;
        };
        let generation = self.begin_form_async(cause);
        let form = *self;
        let task = spawn(async move {
            let result = job.await;
            form.finish_form_async(cause, generation, result);
        });
        self.runtime
            .with(|runtime| runtime.slots[cause.index()].set_task(task));
    }

    pub(crate) fn fire_form_change(&self) {
        let (listener, delay) = self.runtime.with(|runtime| {
            (
                runtime.listeners.on_change.clone(),
                runtime.listeners.on_change_debounce,
            )
        });
        let Some(listener) = listener else {
            return;
        };
        let values = self.values_untracked();
        if delay.is_zero() {
            listener(&values);
            return;
        }
        let generation = self.runtime.with(|runtime| runtime.listener_slot.restart());
        let form = *self;
        let task = spawn(async move {
            timer(delay).await;
            if form
                .runtime
                .with(|runtime| runtime.listener_slot.is_current(generation))
            {
                listener(&values);
            }
        });
        self.runtime
            .with(|runtime| runtime.listener_slot.set_task(task));
    }

    pub(crate) fn fire_form_blur(&self) {
        let listener = self
            .runtime
            .with(|runtime| runtime.listeners.on_blur.clone());
        if let Some(listener) = listener {
            listener(&self.values_untracked());
        }
    }

    /// The registered fields that are array rows under `prefix` at index `from` or later.
    pub(crate) fn rows_from(&self, prefix: &FieldPath, from: usize) -> Vec<Rc<dyn AnyField<H>>> {
        let depth = prefix.segments().len();
        self.registered()
            .into_iter()
            .filter(|field| {
                field.path().is_some_and(|path| {
                    let segments = path.segments();
                    segments.len() > depth
                        && segments[..depth] == *prefix.segments()
                        && matches!(segments[depth], PathSegment::Index(row) if row >= from)
                })
            })
            .collect()
    }

    /// Moves the metadata of the array rows under `prefix`. See [`crate::meta::remap_rows`].
    pub(crate) fn remap_rows(&self, prefix: &FieldPath, remap: impl Fn(usize) -> Option<usize>) {
        self.entries
            .with(|entries| crate::meta::remap_rows(entries, prefix, remap));
        self.bump();
    }
}
