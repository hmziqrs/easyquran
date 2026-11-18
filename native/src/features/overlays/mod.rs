//! Overlays and desktop surfaces: `Dialog`, `AlertDialog`, `Sheet`,
//! `Popover`, `Menu`, `ContextMenu`, `HoverCard`, `Tooltip`, toasts,
//! `DockArea`, resizable panels, `Editor`, `DiffView`, `TextView`. Owned
//! by the overlays area. The shell mounts `overlays_page`; keep its name
//! and signature.

mod code_sections;
mod dialog_section;
mod dock_section;
mod hovering;
mod menus;
mod panels;
mod sheet_section;
mod toasts;

use crate::ui::button as ui_button;
use crate::ui::controls as ui_controls;
use crate::ui::field as ui_field;
use crate::ui::popups as ui_popups;
use nsh_base::ButtonElement;
use nsh_base::theme;
use nsh_core::{element::StyleRefinement, prelude::*};

/// A section's button, forwarded to `crate::ui`'s with this area's
/// variant names for the destructive commitment.
pub(crate) fn overlay_button(
    label: &'static str,
    variant: ButtonVariant,
    on_press: impl Fn() + 'static,
) -> ButtonElement {
    ui_button::button_with(
        label,
        match variant {
            ButtonVariant::Primary => ui_button::ButtonVariant::Primary,
            ButtonVariant::Outline => ui_button::ButtonVariant::Outline,
            ButtonVariant::Ghost => ui_button::ButtonVariant::Ghost,
            ButtonVariant::Destructive => ui_button::ButtonVariant::Danger,
        },
        ui_button::ButtonSize::Medium,
        false,
        None,
        on_press,
    )
}

/// The variants [`overlay_button`] forwards.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum ButtonVariant {
    Primary,
    Outline,
    Ghost,
    Destructive,
}

/// A section's switch row, forwarded to `crate::ui`.
pub(crate) fn overlay_switch(label: &'static str, on: Signal<bool>) -> impl IntoElement {
    ui_controls::switch_row(label, on)
}

/// A section's single-line field, `crate::ui`'s.
pub(crate) fn text_field(value: Signal<String>, placeholder: impl Into<String>) -> nsh_base::Input {
    ui_field::text_field(value, placeholder)
}

/// The popup family's one surface, `crate::ui`'s.
pub(crate) fn popup_surface(s: StyleRefinement) -> StyleRefinement {
    ui_popups::popup_surface(s)
}

/// A scrim over what a dialog or sheet covers, `crate::ui`'s token-backed
/// backdrop.
pub(crate) fn scrim(s: StyleRefinement) -> StyleRefinement {
    ui_popups::popup_backdrop(s)
}

/// Where this page deliberately goes past `crate::ui`'s wrappers, kept as
/// local builders until the extensions below exist (one block each: what
/// the section needs, what the wrapper lacks, the extension that would
/// close the gap):
///
/// - Menu entries with sections and shortcuts. Needs: grouped rows with
///   section labels, a trailing shortcut hint per row, and an explicit
///   accessibility label for rows whose visible text is "label + hint".
///   `ui::popups::menu_entry` styles one flat row from its label alone and
///   has no shortcut slot, no label override and no group labels. An
///   `accessibility_label` parameter, an optional `shortcut: &str` and a
///   plain `group_label` helper would let the sections adopt it.
/// - Sheet with detents. Needs: a bottom panel whose height follows a
///   detent signal (fractions of the viewport) and swipe-to-close on the
///   bottom side only. `ui::sheet::sheet_of` fixes the panel at
///   `w(320).h_full()` for every side, ignores detents, never enables
///   `close_on_swipe`, and folds the close reason away (its
///   `on_open_change` wrapper drops `OpenChangeReason`, so a section
///   cannot tell Escape from a scrim press). A `height`/`max_height`
///   parameter for `Placement::Bottom`, a `close_on_swipe` flag and a
///   reason-preserving `on_open_change` would close the gap.
/// - Toasts with actions and distinct timeouts. Needs: per-toast timeout
///   (some none), an action button that dismisses and reports, a close
///   button, and a readable queue order line. `ui::toast::notify` queues
///   bare strings with a fixed 5-second timeout into the app-level
///   `ToastHost`, with no action, no close, no per-toast options and no
///   queue view. `ToastOptions`-carrying messages (`timeout`, `action`)
///   and a section-scoped host would close the gap; the app-level host
///   stays the shell's.
/// - Dock area with testable tabs and real panels. Needs: per-tab test ids
///   (`tab-<key>`) for drag tests, rich per-panel content (views, not a
///   text string), and the layout reported with the status the section
///   shows. `ui::dock::dock_workspace` renders no test ids and takes
///   `content: SharedString` per panel. A `test_id` on rendered tabs and a
///   render-closure content field would close the gap.
/// - Resizable panels with controlled sizes and bounds. Needs: per-panel
///   `min_size`/`max_size`, the sizes held in the section's signal that
///   step buttons also write, and reactive re-application. `ui::resizable::
///   h_resizable_panels` fixes `min_size(80)`, takes no initial/controlled
///   sizes, and reports only through its own signal. `min_size`/`max_size`
///   on the panel spec plus accepting a controlled `sizes` signal would
///   close the gap.
/// - Editor with an edit hook. Needs: uncontrolled `default_value` text,
///   bounded growth (`min_rows`/`max_rows`) and an edit counter fed by
///   `on_change`. `ui::editor::editor_of` controls `value` and offers no
///   change hook. An optional `on_change` callback and `default_value`
///   when no signal is given would close the gap.
/// - Diff with a layout toggle and stats. Needs: side-by-side and unified
///   layouts switched from the section, and the diff's stats for the
///   status line. `ui::editor::diff_view_of` fixes `DiffLayout::Unified`
///   and drops `on_stats_change`. A `layout` parameter and a stats hook
///   would close the gap.
///
/// Secondary reading inside a section, static or following a signal.
pub(crate) fn caption(note: impl IntoReactive<SharedString> + 'static) -> impl IntoElement {
    text(note)
        .text_xs()
        .text_color(move || theme().muted_foreground)
}

/// One flow of the page: its name over a card holding the flow.
fn framed(title: &'static str, flow: impl IntoElement) -> impl IntoElement {
    view()
        .gap_2()
        .child(crate::ui::text::section_title(title))
        .child(crate::ui::layout::card().child(flow))
}

/// A status line a flow writes to, so tests and people see the outcome.
pub(crate) fn status_line(value: impl Fn() -> String + 'static) -> impl IntoElement {
    text(value)
        .text_xs()
        .font_family(crate::ui::text::mono_family)
        .text_color(move || theme().muted_foreground)
}

/// A trailing shortcut hint of a menu row.
pub(crate) fn shortcut(keys: &'static str) -> impl IntoElement {
    text(keys)
        .text_xs()
        .font_family(crate::ui::text::mono_family)
        .text_color(move || theme().muted_foreground)
}

/// The overlays page: every overlay and desktop surface, each in a
/// section that runs a real flow. Toasts are hosted once, at the end.
pub fn overlays_page() -> impl IntoElement {
    view().flex_1().bg(move || theme().background).child(
        scroll()
            .flex_1()
            .content_inset(crate::ui::layout::page_insets(0.0))
            .child(
                view().w_full().flex_col().items_center().child(
                    crate::ui::layout::page_body()
                        .max_w(px(720.0))
                        .child(crate::ui::text::page_intro(
                            "Overlays",
                            "Dialogs, popups, menus, toasts and desktop surfaces, each in a flow.",
                        ))
                        .child(framed("Dialog", dialog_section::dialog_section()))
                        .child(framed("Alert dialog", dialog_section::alert_section()))
                        .child(framed("Sheet", sheet_section::sheet_section()))
                        .child(framed("Popover", menus::popover_section()))
                        .child(framed("Menu", menus::menu_section()))
                        .child(framed("Context menu", menus::context_menu_section()))
                        .child(framed("Hover card", hovering::hover_card_section()))
                        .child(framed("Tooltip", hovering::tooltip_section()))
                        .child(framed("Toasts", toasts::toasts_area()))
                        .child(framed("Dock area", dock_section::dock_section()))
                        .child(framed("Resizable panels", panels::panels_section()))
                        .child(framed("Editor", code_sections::editor_section()))
                        .child(framed("Diff view", code_sections::diff_section()))
                        .child(framed("Text view", code_sections::text_section())),
                ),
            ),
    )
}

/// A root that mounts just this page, so `tests/overlays.rs` runs it
/// without the shell.
#[component]
pub fn OverlaysTestRoot() -> impl IntoElement {
    overlays_page()
}
