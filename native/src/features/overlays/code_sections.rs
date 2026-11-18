//! The desktop text surfaces: an `Editor` with syntax highlighting over a
//! small sample, a `DiffView` over two versions of it, and a `TextView`
//! rendering long styled text.

use super::{ButtonVariant, caption, overlay_button, status_line};
use nsh_base::text_view::PartStyle;
use nsh_base::{
    DiffLayout, DiffStats, TextViewClasses, TextViewStyles, diff_view, editor, text_view, theme,
};
use nsh_core::{
    element::{Style, StyleRefinement},
    prelude::*,
};

/// The sample the editor starts from.
const SAMPLE: &str = r#"// The pipeline's retry policy.
pub fn retries(plan: &Plan) -> usize {
    match plan.kind {
        Kind::Interactive => 0,
        Kind::Nightly => 3,
    }
}"#;

/// The same sample, edited: a guard clause and a new arm.
const SAMPLE_EDITED: &str = r#"// The pipeline's retry policy.
pub fn retries(plan: &Plan) -> usize {
    if plan.disabled {
        return 0;
    }
    match plan.kind {
        Kind::Interactive => 1,
        Kind::Nightly => 3,
    }
}"#;

/// A part's inline style, made the way a style prop's closure is.
fn part_style(f: impl FnOnce(StyleRefinement) -> StyleRefinement) -> PartStyle {
    PartStyle(Style::from(f(StyleRefinement::default())))
}

/// `Editor` with syntax highlighting, line numbers and folding over a
/// small Rust sample; the change count proves edits reach the app.
pub(crate) fn editor_section() -> impl IntoElement {
    let changes = signal(0_u32);
    view()
        .flex_col()
        .gap_3()
        .child(
            editor()
                .language("rust")
                .default_value(SAMPLE)
                .soft_wrap(true)
                .min_rows(6)
                .max_rows(12)
                .accessibility_label("Sample editor")
                .on_change(move |_: &SharedString| {
                    changes.update(|count| *count += 1);
                }),
        )
        .child(status_line(move || format!("Edits: {}", changes.get())))
        .child(caption(
            "Highlighting, folding and the active line come with the editor; the app styles its parts.",
        ))
}

/// `DiffView` over two versions of the sample, switchable between the
/// split and unified layouts, with the diff's stats.
pub(crate) fn diff_section() -> impl IntoElement {
    let layout = signal(DiffLayout::Split);
    let stats = signal("—".to_owned());
    let set_split = move || layout.set(DiffLayout::Split);
    let set_unified = move || layout.set(DiffLayout::Unified);
    view()
        .flex_col()
        .gap_3()
        .child(
            view()
                .flex_row()
                .gap_2()
                .child(
                    when(
                        move || layout.get() == DiffLayout::Split,
                        move || overlay_button("Side by side", ButtonVariant::Primary, set_split),
                    )
                    .otherwise(move || {
                        overlay_button("Side by side", ButtonVariant::Outline, set_split)
                    }),
                )
                .child(
                    when(
                        move || layout.get() == DiffLayout::Unified,
                        move || overlay_button("Unified", ButtonVariant::Primary, set_unified),
                    )
                    .otherwise(move || {
                        overlay_button("Unified", ButtonVariant::Outline, set_unified)
                    }),
                ),
        )
        .child(
            diff_view()
                .old_value(SAMPLE)
                .new_value(SAMPLE_EDITED)
                .language("rust")
                .layout(move || layout.get())
                .line_height(px(20.0))
                .h(px(300.0))
                .on_stats_change(move |s: &DiffStats| {
                    stats.set(format!("+{} −{} in {} changes", s.added, s.deleted, s.hunks));
                }),
        )
        .child(status_line({ let s = stats; move || s.get() }))
        .child(caption(
            "Changed words are highlighted within the lines; unchanged runs collapse until expanded.",
        ))
}

/// `TextView` over a long styled document, with the app's part classes.
pub(crate) fn text_section() -> impl IntoElement {
    let last_link = signal("No link yet".to_owned());
    view()
        .flex_col()
        .gap_3()
        .child(
            text_view()
                .markdown(LONG_DOCUMENT)
                .selectable(true)
                .classes(TextViewClasses {
                    heading1: Some("text-xl font-bold".into()),
                    heading2: Some("text-lg font-semibold".into()),
                    code: Some("font-mono text-xs".into()),
                    blockquote: Some("border-l-2 pl-3".into()),
                    link: Some("underline".into()),
                    ..Default::default()
                })
                .styles(TextViewStyles {
                    heading1: Some(part_style(|s| s.text_color(move || theme().foreground))),
                    heading2: Some(part_style(|s| s.text_color(move || theme().foreground))),
                    code_block: Some(part_style(|s| {
                        s.border_1()
                            .border_color(move || theme().border)
                            .bg(move || theme().muted.opacity(0.35))
                    })),
                    blockquote: Some(part_style(|s| s.border_color(move || theme().primary))),
                    ..Default::default()
                })
                .on_link_press({
                                        move |e: &nsh_base::TextViewLinkPressEvent| {
                        last_link.set(format!("Link: {}", e.url));
                    }
                }),
        )
        .child(status_line({ let s = last_link; move || s.get() }))
        .child(caption(
            "Markdown with tables, task lists and alerts, styled part by part with the theme's tokens.",
        ))
}

/// The long document the text view renders.
const LONG_DOCUMENT: &str = "\
# Style guide

This document exercises the text view: **bold**, *emphasis*, `inline
code`, [a link](https://example.com/style), and a list.

## Writing

1. Name the object.
2. Say the consequence.
3. Offer the way back.

- [x] Ship the dialog
- [ ] Review the diff

> A rule with one decision per line reads faster than a paragraph.

```rust
let theme = Theme::of(tokens);
```

| Surface | Token |
| --- | --- |
| Popup | `popover` |
| Card | `card` |

> [!NOTE]
> Alerts come from GitHub's Markdown, and this one is styled through the
> alert note part.
";
