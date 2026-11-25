//! Typed forms for Native Shell Rust apps, after TanStack Form. Values live in
//! one struct. Validation runs on events and keeps its errors per cause. Dirty
//! tracking and field paths come from `gpui-form`'s GPUI-free core, and
//! everything a screen reads is a signal, so its bindings update by themselves.

mod array;
mod bind;
mod field;
mod form;
mod listeners;
mod meta;
pub mod rules;
mod schema;
mod validation;

pub use array::FieldArray;
pub use bind::BindField;
pub use field::{Field, FieldValidators};
pub use form::{
    AllErrors, Form, FormDiagnostics, FormOptions, FormSnapshot, FormStateSnapshot, FormValidators,
    SubmitError, SubmitResult, form_options, use_form, use_form_with,
};
pub use gpui_form_core::FieldPath;
pub use listeners::{FieldListeners, FormListeners, Listeners};
pub use meta::FieldMeta;
pub use schema::{AsyncSchema, Schema, SchemaIssue};
pub use validation::{
    ErrorMap, FormValidationResult, ValidationCause, ValidationError, ValidationLogic,
    ValidationTrigger,
};

/// A field from a path of the form's value: `field!(form, address.city)`.
#[macro_export]
macro_rules! field {
    ($form:expr, $($segment:ident).+) => {
        $form.field(
            &[$(stringify!($segment)),+],
            |values| &values.$($segment).+,
            |values| &mut values.$($segment).+,
        )
    };
}

/// A list field from a path of the form's value: `array_field!(form, items)`.
/// Its rows are `FieldArray` items, see [`FieldArray::item_field`].
#[macro_export]
macro_rules! array_field {
    ($form:expr, $($segment:ident).+) => {
        $form.array_field(
            &[$(stringify!($segment)),+],
            |values| &values.$($segment).+,
            |values| &mut values.$($segment).+,
        )
    };
}
