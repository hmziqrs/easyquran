//! Schemas: a checker of a whole value that reports issues by path. A schema is a plain
//! Rust type that implements [`Schema`]. No schema library is needed.

use std::future::Future;
use std::pin::Pin;

use crate::validation::FormValidationResult;

/// One problem a schema found. `path` names the field it applies to, dotted as in
/// [`crate::FieldPath`]; `""` names the value as a whole, or the form.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaIssue {
    /// The path of the field the issue applies to. Empty for the whole value.
    pub path: String,
    /// The message to show.
    pub message: String,
}

impl SchemaIssue {
    /// An issue for the whole value.
    pub fn form(message: impl Into<String>) -> Self {
        Self {
            path: String::new(),
            message: message.into(),
        }
    }

    /// An issue for the field at `path`.
    pub fn field(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

/// Checks a value. An empty list means the value is valid.
pub trait Schema<T> {
    /// The issues in `value`, or none.
    fn check(&self, value: &T) -> Vec<SchemaIssue>;
}

/// Checks a value asynchronously, for example against a server. Same meaning as [`Schema`].
pub trait AsyncSchema<T> {
    /// The issues in `value`, or none. The future owns its input, so it may outlive the caller.
    fn check(&self, value: T) -> Pin<Box<dyn Future<Output = Vec<SchemaIssue>>>>;
}

/// The first issue's message for the value itself, for a field validator.
pub(crate) fn first_message<T>(schema: &dyn Schema<T>, value: &T) -> Option<String> {
    schema
        .check(value)
        .into_iter()
        .next()
        .map(|issue| issue.message)
}

/// Turns a schema's issues into a form validation result: whole-value issues become the form's
/// message, and the others become field messages.
pub(crate) fn to_result(issues: Vec<SchemaIssue>) -> FormValidationResult {
    let mut result = FormValidationResult::default();
    for issue in issues {
        if issue.path.is_empty() {
            if result.form.is_none() {
                result.form = Some(issue.message);
            }
        } else {
            result.fields.push((issue.path, issue.message));
        }
    }
    result
}
