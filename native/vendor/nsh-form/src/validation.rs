//! Validation causes, error maps, and the plan that picks which validators run
//! for an event. Errors live per cause, as in TanStack's `errorMap`.

use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

/// The message a validator returns for a value that fails.
pub type ValidationError = String;

pub(crate) const CAUSE_COUNT: usize = 6;

/// What a validator ran for. Each cause has its own slot in an [`ErrorMap`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ValidationCause {
    /// The value changed.
    Change,
    /// The field lost focus.
    Blur,
    /// A submit ran.
    Submit,
    /// The field or form was mounted.
    Mount,
    /// An answer from the server on submit. The next validation clears it.
    Server,
    /// Revalidation after a submit, see [`ValidationLogic::Revalidate`].
    Dynamic,
}

impl ValidationCause {
    /// Causes in the order their messages are listed. A server error is shown first.
    const DISPLAY_ORDER: [ValidationCause; CAUSE_COUNT] = [
        ValidationCause::Server,
        ValidationCause::Mount,
        ValidationCause::Change,
        ValidationCause::Blur,
        ValidationCause::Submit,
        ValidationCause::Dynamic,
    ];

    pub(crate) fn index(self) -> usize {
        self as usize
    }
}

/// One optional error per [`ValidationCause`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ErrorMap {
    slots: [Option<ValidationError>; CAUSE_COUNT],
}

impl ErrorMap {
    /// An empty map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the error of one cause and returns the map, for building one in place.
    pub fn with(mut self, cause: ValidationCause, error: impl Into<ValidationError>) -> Self {
        self.set(cause, Some(error.into()));
        self
    }

    /// The error of one cause.
    pub fn get(&self, cause: ValidationCause) -> Option<&str> {
        self.slots[cause.index()].as_deref()
    }

    /// Sets or clears the error of one cause.
    pub fn set(&mut self, cause: ValidationCause, error: Option<ValidationError>) {
        self.slots[cause.index()] = error;
    }

    /// Whether no cause has an error.
    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(Option::is_none)
    }

    /// Each cause that has an error, with the error, in display order.
    pub fn iter(&self) -> impl Iterator<Item = (ValidationCause, &str)> + '_ {
        ValidationCause::DISPLAY_ORDER
            .into_iter()
            .filter_map(move |cause| self.get(cause).map(|error| (cause, error)))
    }

    /// The distinct messages in display order. One rule that runs on change,
    /// blur and submit gives one message, not three.
    pub fn messages(&self) -> Vec<ValidationError> {
        let mut out: Vec<ValidationError> = Vec::new();
        for (_, message) in self.iter() {
            if !out.iter().any(|seen| seen == message) {
                out.push(message.to_owned());
            }
        }
        out
    }

    pub(crate) fn first(&self) -> Option<ValidationError> {
        self.iter().next().map(|(_, message)| message.to_owned())
    }

    /// This map, with `base` filling every cause this one leaves empty.
    pub(crate) fn over(&self, base: &ErrorMap) -> ErrorMap {
        let mut out = ErrorMap::default();
        for cause in ValidationCause::DISPLAY_ORDER {
            let message = self.get(cause).or(base.get(cause)).map(str::to_owned);
            out.set(cause, message);
        }
        out
    }
}

/// Where a validation pass starts. Revalidation picks its causes from this.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidationTrigger {
    /// A value change.
    Change,
    /// A blur.
    Blur,
    /// A submit.
    Submit,
}

/// Which validators run for an event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ValidationLogic {
    /// Each event runs the validators of its own cause.
    #[default]
    Default,
    /// TanStack's `revalidateLogic`: the dynamic validators run from `mode`
    /// until the first submit, then from `mode_after_submission`. The default
    /// validators of the event run as well.
    Revalidate {
        mode: ValidationTrigger,
        mode_after_submission: ValidationTrigger,
    },
}

impl ValidationLogic {
    /// TanStack's defaults: dynamic validators run on submit, then on change.
    pub fn revalidate() -> Self {
        Self::Revalidate {
            mode: ValidationTrigger::Submit,
            mode_after_submission: ValidationTrigger::Change,
        }
    }
}

/// What happened to a field or form, from the form's side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Event {
    Mount,
    Change,
    Blur,
    Submit,
}

impl Event {
    /// The event that runs the validators of `cause`. Server and dynamic causes run on submit.
    pub(crate) fn of_cause(cause: ValidationCause) -> Event {
        match cause {
            ValidationCause::Change => Event::Change,
            ValidationCause::Blur => Event::Blur,
            ValidationCause::Mount => Event::Mount,
            ValidationCause::Submit | ValidationCause::Server | ValidationCause::Dynamic => {
                Event::Submit
            }
        }
    }

    fn trigger(self) -> Option<ValidationTrigger> {
        match self {
            Event::Change => Some(ValidationTrigger::Change),
            Event::Blur => Some(ValidationTrigger::Blur),
            Event::Submit => Some(ValidationTrigger::Submit),
            Event::Mount => None,
        }
    }
}

/// The causes an event runs, in order. Server clears its error on every
/// change, blur and submit, as TanStack's `onServer` does.
pub(crate) fn plan(event: Event, logic: ValidationLogic, attempts: u32) -> Vec<ValidationCause> {
    let mut causes = match event {
        Event::Mount => vec![ValidationCause::Mount],
        Event::Change => vec![ValidationCause::Change, ValidationCause::Server],
        Event::Blur => vec![ValidationCause::Blur, ValidationCause::Server],
        Event::Submit => vec![
            ValidationCause::Change,
            ValidationCause::Blur,
            ValidationCause::Submit,
            ValidationCause::Server,
        ],
    };
    if let ValidationLogic::Revalidate {
        mode,
        mode_after_submission,
    } = logic
    {
        let watched = if attempts == 0 {
            mode
        } else {
            mode_after_submission
        };
        if event == Event::Submit || event.trigger() == Some(watched) {
            causes.push(ValidationCause::Dynamic);
        }
    }
    causes
}

/// What a form-level validator, or a submit, reports: a message for the form,
/// and messages for fields, named by path (`"address.city"`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FormValidationResult {
    /// A message for the form as a whole.
    pub form: Option<String>,
    /// Messages for fields, named by path.
    pub fields: Vec<(String, String)>,
}

impl FormValidationResult {
    /// No errors.
    pub fn ok() -> Self {
        Self::default()
    }

    /// A message for the form as a whole.
    pub fn form(message: impl Into<String>) -> Self {
        Self {
            form: Some(message.into()),
            fields: Vec::new(),
        }
    }

    /// A message for one field, named by its path: `"address.city"`.
    pub fn field(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            form: None,
            fields: vec![(path.into(), message.into())],
        }
    }

    /// Adds a message for another field.
    pub fn and_field(mut self, path: impl Into<String>, message: impl Into<String>) -> Self {
        self.fields.push((path.into(), message.into()));
        self
    }

    pub(crate) fn field_message(&self, path: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(name, _)| name == path)
            .map(|(_, message)| message.as_str())
    }
}

pub(crate) type Rule<H, T> = Rc<dyn Fn(&T, &H) -> Option<ValidationError>>;
pub(crate) type AsyncRule<T> =
    Rc<dyn Fn(T) -> Pin<Box<dyn Future<Output = Option<ValidationError>>>>>;
pub(crate) type FormRule<H> = Rc<dyn Fn(&H) -> FormValidationResult>;
pub(crate) type AsyncFormRule<H> =
    Rc<dyn Fn(H) -> Pin<Box<dyn Future<Output = FormValidationResult>>>>;
pub(crate) type Job = Pin<Box<dyn Future<Output = Option<ValidationError>>>>;
pub(crate) type FormJob = Pin<Box<dyn Future<Output = FormValidationResult>>>;

pub(crate) fn boxed<T>(
    future: impl Future<Output = T> + 'static,
) -> Pin<Box<dyn Future<Output = T>>> {
    Box::pin(future)
}
