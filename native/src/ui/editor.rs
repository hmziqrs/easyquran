//! Long-form text: the code editor, the diff of two texts, and the
//! markdown document, all with their parts styled here.

use nsh_base::rich_editor::view::{RichEditorClasses, rich_editor};
use nsh_base::{DiffLayout, TextViewClasses, diff_view, editor, text_view, theme};
use nsh_core::prelude::*;

/// A code editor for `value`, with line numbers and the app's surface.
/// It scrolls itself; give it a bounded height.
pub fn editor_of(
    name: &'static str,
    value: Signal<SharedString>,
    language: &'static str,
) -> impl IntoElement {
    editor()
        .accessibility_label(name)
        .value(move || Some(value.get()))
        .on_change(move |next: &SharedString| value.set(next.clone()))
        .language(language)
        .line_numbers(true)
        .soft_wrap(true)
        .font_size(px(13.0))
        .flex_1()
        .rounded(move || theme().radius)
        .border_1()
        .border_color(move || theme().border)
        .bg(move || theme().card)
        .text_color(move || theme().card_foreground)
}

/// The changes between two texts, unified, with the app's line colors.
pub fn diff_view_of(old_value: SharedString, new_value: SharedString) -> impl IntoElement {
    // The classes struct is `#[non_exhaustive]`, so fields are assigned
    // after `default()` instead of a literal.
    #[allow(clippy::field_reassign_with_default)]
    let mut classes = nsh_base::DiffViewClasses::default();
    classes.side = Some("bg-card rounded-lg border border-border overflow-hidden".into());
    classes.line = Some("px-3 text-sm text-card-foreground".into());
    classes.line_added = Some("text-success bg-success/10".into());
    classes.line_deleted = Some("text-destructive bg-destructive/10".into());
    diff_view()
        .old_value(old_value)
        .new_value(new_value)
        .layout(DiffLayout::Unified)
        .line_numbers(true)
        .classes(classes)
        .font_size(px(13.0))
        .text_color(move || theme().card_foreground)
}

/// A markdown document, selectable, in the body type; links stay
/// reported, not opened, until the screen decides.
pub fn markdown_of(markdown: impl IntoReactive<SharedString> + 'static) -> impl IntoElement {
    let markdown = markdown.into_reactive();
    text_view()
        .markdown(move || Some(markdown.get()))
        .selectable(true)
        .flex_col()
        .gap_3()
        .text_sm()
        .text_color(move || theme().foreground)
        .classes(TextViewClasses {
            heading1: Some("text-xl font-semibold text-foreground".into()),
            heading2: Some("text-base font-semibold text-foreground".into()),
            code: Some("text-xs font-mono bg-muted rounded px-1".into()),
            code_block: Some("bg-muted rounded-md p-3".into()),
            link: Some("text-primary underline".into()),
            ..TextViewClasses::default()
        })
}

/// A rich text document editor (headings, lists, marks, Markdown
/// shortcuts) over `content`, ProseMirror's JSON. Its surface is the
/// app's; reading the document back goes through the session, so edits
/// live in the editor until the screen asks for it.
pub fn rich_editor_of(content: serde_json::Value, placeholder: &'static str) -> impl IntoElement {
    // `#[non_exhaustive]`: assign fields on `default()` (see above).
    #[allow(clippy::field_reassign_with_default)]
    let mut classes = RichEditorClasses::default();
    classes.root = Some("bg-card text-card-foreground".into());
    classes.content = Some("text-sm leading-relaxed".into());
    classes.placeholder = Some("text-muted-foreground".into());
    rich_editor()
        .content(Some(content))
        .placeholder(placeholder)
        .min_h(px(160.0))
        .p_3()
        .rounded(move || theme().radius)
        .border_1()
        .border_color(move || theme().border)
        .bg(move || theme().card)
        .classes(classes)
}
