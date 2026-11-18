//! The pointer-following previews: `HoverCard` (a person's card on hover
//! and keyboard focus) and `Tooltip` (a hint on hover, or when the app
//! opens it on touch screens).

use super::{ButtonVariant, caption, overlay_button, popup_surface, status_line};
use nsh_base::{HoverCardEvent, OpenChangeReason, TooltipEvent, hover_card, theme, tooltip};
use nsh_core::prelude::*;

/// The initials chip a hover card is anchored to.
fn mention_chip(test_id: &'static str) -> impl IntoElement {
    view()
        .min_h_9()
        .px_3()
        .py_2()
        .flex_row()
        .items_center()
        .rounded_full()
        .bg(move || theme().accent)
        .transition_colors()
        .active(|s| s.bg(move || theme().muted))
        .test_id(test_id)
        .role(AccessibilityRole::Button)
        .child(
            text("@mchen")
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .text_color(move || theme().accent_foreground),
        )
}

/// An avatar square, shared by the chip and the card.
fn avatar(initials: &'static str, size: f32) -> impl IntoElement {
    view()
        .size(px(size))
        .rounded_full()
        .items_center()
        .justify_center()
        .bg(move || theme().primary)
        .child(
            text(initials)
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(move || theme().primary_foreground),
        )
}

/// `HoverCard`: previews what its trigger points at, opening after the
/// pointer rests (or keyboard focus stays), staying while the pointer
/// moves onto the card.
pub(crate) fn hover_card_section() -> impl IntoElement {
    let open = signal(false);
    let following = signal(false);
    let status = signal("Closed".to_owned());
    view()
        .flex_col()
        .gap_3()
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_3()
                .child(
                    hover_card()
                        .open(move || Some(open.get()))
                        .on_open_change({
                                                        move |e: &HoverCardEvent| {
                                if let HoverCardEvent::OpenChange(next, reason) = e {
                                    // Controlled: the card follows the
                                    // pointer's and focus's requests.
                                    open.set(*next);
                                    let why = match reason {
                                        OpenChangeReason::Hover => "hover",
                                        OpenChangeReason::Focus => "focus",
                                        OpenChangeReason::TriggerPress => "tap",
                                        _ => "dismiss",
                                    };
                                    status.set(format!(
                                        "{} ({})",
                                        if *next { "opened" } else { "closed" },
                                        why
                                    ));
                                }
                            }
                        })
                        .placement(Placement::Bottom)
                        .align(Align::Start)
                        .offset(px(6.0))
                        .margin(px(8.0))
                        .open_delay(150.0)
                        .close_delay(200.0)
                        .popup_style(popup_surface)
                        .trigger(
                            view()
                                .flex_row()
                                .items_center()
                                .gap_2()
                                .child(mention_chip("hover-card-trigger"))
                                .child(avatar("MC", 32.0)),
                        )
                        .child(
                            view()
                                .w(px(280.0))
                                .flex_col()
                                .gap_3()
                                .p_4()
                                .child(
                                    view()
                                        .flex_row()
                                        .items_center()
                                        .gap_3()
                                        .child(avatar("MC", 44.0))
                                        .child(
                                            view()
                                                .flex_col()
                                                .gap_1()
                                                .child(
                                                    text("Mira Chen")
                                                        .text_base()
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_color(move || {
                                                            theme().popover_foreground
                                                        }),
                                                )
                                                .child(
                                                    text("Design lead · Platform")
                                                        .text_xs()
                                                        .text_color(move || {
                                                            theme().muted_foreground
                                                        }),
                                                ),
                                        ),
                                )
                                .child(
                                    text("Shapes the components teams build on. Ask about motion and type.")
                                        .text_sm()
                                        .text_color(move || theme().popover_foreground),
                                )
                                .child(
                                    overlay_button("Follow", ButtonVariant::Primary, move || {
                                        following.set(!following.get_untracked())
                                    })                                    ,
                                ),
                        ),
                )
                .child(
                    text("mentioned you in Q3 roadmap")
                        .text_sm()
                        .text_color(move || theme().muted_foreground),
                ),
        )
        .child(caption(
            "Opens on a short hover or keyboard focus; moving onto the card keeps it open.",
        ))
        .child(status_line({ let s = status; move || s.get() }))
        .child(status_line(move || {
            if following.get() {
                "Following Mira Chen".to_owned()
            } else {
                "Not following".to_owned()
            }
        }))
}

/// `Tooltip`: a hint attached to an element, shown after the pointer
/// rests; touch screens show it only when the app opens it, so this one
/// is controlled and follows both.
pub(crate) fn tooltip_section() -> impl IntoElement {
    let shown = signal(false);
    let status = signal("Hint hidden".to_owned());
    view()
        .flex_col()
        .gap_3()
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_3()
                .child(
                    view()
                        .min_h_9()
                        .px_4()
                        .py_2()
                        .flex_row()
                        .items_center()
                        .rounded(move || theme().radius)
                        .bg(move || theme().primary)
                        .transition_colors()
                        .active(|s| s.bg(move || theme().primary.opacity(0.85)))
                        .test_id("save-chip")
                        .tooltip(
                            tooltip()
                                .placement(Placement::Top)
                                .open(move || Some(shown.get()))
                                .on_open_change({
                                    move |e: &TooltipEvent| {
                                        if let TooltipEvent::OpenChange(next, _) = e {
                                            // Hover requests follow through;
                                            // the toggle button drives the
                                            // same value on touch screens.
                                            shown.set(*next);
                                            status.set(if *next {
                                                "Hint shown".to_owned()
                                            } else {
                                                "Hint hidden".to_owned()
                                            });
                                        }
                                    }
                                })
                                .starting_style(|s| s.opacity(0.0))
                                .child(
                                    text("Saves the draft (⌘S)")
                                        .text_xs()
                                        .px_2()
                                        .py_1()
                                        .rounded(move || theme().radius)
                                        .bg(move || theme().popover)
                                        .border_1()
                                        .border_color(move || theme().border)
                                        .shadow_sm()
                                        .text_color(move || theme().popover_foreground),
                                ),
                        )
                        .child(
                            text("Save")
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(move || theme().primary_foreground),
                        ),
                )
                .child(overlay_button("Toggle hint", ButtonVariant::Outline, {
                    move || {
                        // The app-driven open is the touch path; it
                        // reports like the pointer path does.
                        shown.set(!shown.get_untracked());
                        status.set(if shown.get_untracked() {
                            "Hint shown".to_owned()
                        } else {
                            "Hint hidden".to_owned()
                        });
                    }
                })),
        )
        .child(caption(
            "Hover the Save button for the hint; touch screens show it only when the app opens it.",
        ))
        .child(status_line({
            let s = status;
            move || s.get()
        }))
}
