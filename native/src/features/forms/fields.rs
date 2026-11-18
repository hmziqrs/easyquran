//! The forms feature's field glue: the layout a `nsh-form` field needs
//! (label, control, error) over the ui library's primitives, and the input
//! look with the failed state the ui signal-based wrappers cannot express.
//! The Field-bound variants of `ui::field` are wanted upstream of this
//! module; the request is recorded in docs/STATUS.md (lands with the
//! forms area's STATUS section). No raw `nsh_base` control leaves it.

use nsh_base::{Input, input, theme};
use nsh_core::prelude::*;

use crate::ui::text;

/// A section of fields: the ui card's box (rebuilt here: `ui::layout::card`
/// returns `impl IntoElement`, which takes no children) and its title.
pub fn field_section(title: &'static str, fields: impl IntoIterator<Item = View>) -> View {
    view()
        .w_full()
        .flex_col()
        .gap_3()
        .rounded_lg()
        .border_1()
        .border_color(move || theme().border)
        .bg(move || theme().card)
        .p_4()
        .text_color(move || theme().card_foreground)
        .child(text::section_title(title))
        .children(fields)
}

/// A field's label.
pub fn field_label(label: &'static str) -> AnyElement {
    text::label(label).into_element()
}

/// A live hint under a label (`""` hides it): the age, "Checking…".
pub fn hint_line(hint: impl Fn() -> SharedString + Copy + 'static) -> impl IntoElement {
    when(move || !hint().is_empty(), move || text::muted(hint))
}

/// The message a field shows, once its field has one to give.
pub fn error_line(error: impl Fn() -> Option<String> + Copy + 'static) -> impl IntoElement {
    when(
        move || error().is_some(),
        move || {
            text(move || error().unwrap_or_default())
                .text_sm()
                .text_color(move || theme().destructive)
        },
    )
}

/// A labelled field: label, the control, its error.
pub fn field_row(
    label: &'static str,
    control: impl IntoElement,
    error: impl Fn() -> Option<String> + Copy + 'static,
) -> View {
    revealed_row(
        view()
            .flex_col()
            .gap_1p5()
            .child(field_label(label))
            .child(control)
            .child(error_line(error)),
        move || error().is_some(),
    )
}

/// Room kept above a row the failed submit scrolls to: the platform's own
/// reveal margin, so the label sits where the input alone would have.
const ROW_REVEAL_MARGIN: f32 = 16.0;

/// Scrolls the first failing row, label and all, into view after a failed
/// submit. nsh-form focuses the first invalid input there, and the
/// platform's focus reveal brings only that input into view, which leaves
/// the label above it under the page header. A screen makes one, wraps
/// its scroll and content with it, and `provide`s it; every row made by
/// [`field_row`] (or passed to [`revealed_row`]) under it then registers.
#[derive(Clone)]
pub struct RowReveal {
    scroll: nsh_core::element::ElementRef,
    content: nsh_core::element::ElementRef,
    /// The scroll's top content inset, which `scroll_to` adds back.
    inset_top: f32,
    rows: std::rc::Rc<std::cell::RefCell<Vec<RevealEntry>>>,
}

type RevealEntry = (nsh_core::element::ElementRef, std::rc::Rc<dyn Fn() -> bool>);

impl RowReveal {
    /// One per screen, for a scroll with `inset_top` points of top inset;
    /// later rows find it through the context.
    pub fn provide<H: Clone + PartialEq + 'static>(
        form: nsh_form::Form<H>,
        inset_top: f32,
    ) -> Self {
        let reveal = Self {
            scroll: element_ref(),
            content: element_ref(),
            inset_top,
            rows: Default::default(),
        };
        provide_context(reveal.clone());
        let hook = reveal.clone();
        form.on_submit_invalid(move |_: &H| hook.reveal_first_failing());
        reveal
    }

    /// The page's scroll view.
    pub fn scroll(&self, scroll: nsh_core::element::Scroll) -> nsh_core::element::Scroll {
        scroll.element_ref(&self.scroll)
    }

    /// The scroll's one content child.
    pub fn content(&self, content: View) -> View {
        content.element_ref(&self.content)
    }

    /// Scrolls to the first row (in page order) whose field fails. The
    /// form's focus went out before this runs, so this scroll lands after
    /// the platform's reveal and replaces it.
    fn reveal_first_failing(&self) {
        let failing = self
            .rows
            .borrow()
            .iter()
            .find(|(_, failed)| failed())
            .map(|(row, _)| *row);
        let Some(row) = failing else { return };
        let (scroll, content, inset_top) = (self.scroll, self.content, self.inset_top);
        spawn(async move {
            let (Some(row), Some(content)) = (row.measure().await, content.measure().await) else {
                return;
            };
            let top = row.page.y.0 - content.page.y.0 - ROW_REVEAL_MARGIN - inset_top;
            scroll.scroll_to(Point::new(px(0.0), px(top.max(-inset_top))), true);
        });
    }
}

/// Registers a field row with the screen's [`RowReveal`], when it has one.
pub fn revealed_row(row: View, failed: impl Fn() -> bool + 'static) -> View {
    let Some(reveal) = use_context::<RowReveal>() else {
        return row;
    };
    let target = element_ref();
    reveal
        .rows
        .borrow_mut()
        .push((target, std::rc::Rc::new(failed)));
    row.element_ref(&target)
}

/// The app's text input look, as `ui::field`'s frame draws it, with the
/// border reading destructive while the field fails. Behavior (value,
/// autofill, the form's binding) belongs to the caller, then
/// `.bind(field)`.
pub fn input_shell(failed: impl Fn() -> bool + Copy + 'static) -> Input {
    let touch = !is_desktop();
    input()
        .min_h(move || px(if touch { 44.0 } else { 36.0 }))
        .px_3()
        .py_2()
        .text_sm()
        .text_color(move || theme().foreground)
        .rounded(move || theme().radius)
        .border_1()
        .border_color(move || {
            if failed() {
                theme().destructive
            } else {
                theme().input
            }
        })
        .bg(move || theme().background)
        .transition_colors()
        .focus(move |s| {
            s.border_color(move || {
                if failed() {
                    theme().destructive
                } else {
                    theme().ring
                }
            })
            .outline(px(2.0))
            .outline_offset(px(2.0))
            .outline_color(move || theme().ring)
        })
        .placeholder_color(move || color(theme().muted_foreground))
        .caret_color(move || color(theme().foreground))
}

/// The app's header for the desktops and the web (iOS and Android draw the
/// system one from `title`): its back button pops the feature's own stack,
/// so only a page the stack can pop passes `on_back`.
pub fn page_header(title: &'static str, on_back: Option<impl Fn() + 'static>) -> View {
    view()
        .min_h(px(40.0))
        .flex_row()
        .items_center()
        .gap_1()
        .px_3()
        .border_b_1()
        .border_color(move || theme().border)
        .bg(move || theme().background)
        .children(on_back.map(|on_back| {
            crate::ui::button::icon_svg_button(crate::ui::icons::BACK, "Back", on_back)
        }))
        .child(text::label(title))
}

/// The banner a form-level message shows in; `tone` picks the role color.
pub fn message_banner(
    message: impl Fn() -> SharedString + 'static,
    tone: impl Fn() -> AdaptiveColor + Copy + 'static,
) -> impl IntoElement {
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .px_3()
        .py_2()
        .rounded(move || theme().radius)
        .bg(move || theme().muted)
        .border_1()
        .border_color(tone)
        .child(
            text(move || message().to_string())
                .text_sm()
                .text_color(tone),
        )
}

/// The forms' one validation timing (`ValidationLogic::revalidate`): a rule
/// stays quiet until the first submit, then follows every edit. Blur alone
/// never shows an error.
pub trait SubmitRule<H, T> {
    fn check(self, rule: impl Fn(&T, &H) -> Option<String> + 'static) -> Self;
}

impl<H: Clone + PartialEq + 'static, T: Clone + PartialEq + 'static> SubmitRule<H, T>
    for nsh_form::Field<H, T>
{
    fn check(self, rule: impl Fn(&T, &H) -> Option<String> + 'static) -> Self {
        self.validators(nsh_form::FieldValidators::new().on_dynamic(rule))
    }
}

/// A form whose rules run on submit, then on change (see [`SubmitRule`]).
pub fn submit_first_form<H: Clone + PartialEq + 'static>(initial: H) -> nsh_form::Form<H> {
    nsh_form::use_form_with(
        nsh_form::FormOptions::new(initial)
            .validation_logic(nsh_form::ValidationLogic::revalidate()),
    )
}
