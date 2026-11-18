//! The `Sheet` section: notification settings in a panel at an edge — a
//! bottom sheet at phone width, a side panel at desktop width.

use super::{ButtonVariant, caption, overlay_button, overlay_switch, scrim, status_line};
use nsh_base::{OpenChangeReason, SheetEvent, sheet, theme};
use nsh_core::prelude::*;

/// The panel's look for the side it is on: a rounded sheet at the bottom
/// (its height is the current detent, on the panel itself — the style the
/// base sheet reads), a fixed lane at the sides.
fn panel_look(
    side: Placement,
    detent: Signal<u8>,
    s: nsh_core::element::StyleRefinement,
) -> nsh_core::element::StyleRefinement {
    let base = s
        .bg(move || theme().background)
        .border_color(move || theme().border)
        .flex_col()
        .gap_4()
        .p_5()
        .shadow_lg();
    match side {
        Placement::Bottom => base
            .rounded_t(move || theme().radius.max(px(16.0)))
            .border_t_1()
            .pb(move || px(safe_area().bottom.0 + 20.0))
            .h(move || detent_height(detent)),
        _ => base.w(px(340.0)).border_l_1(),
    }
}

/// One detent step of the bottom sheet: fractions of the viewport height.
fn detent_height(detent: Signal<u8>) -> Points {
    let height = window().height.0;
    match detent.get() {
        0 => Points(height * 0.4),
        2 => Points(height * 0.9),
        _ => Points(height * 0.6),
    }
}

/// A close reason's name for the status line.
fn reason_name(reason: OpenChangeReason) -> &'static str {
    match reason {
        OpenChangeReason::TriggerPress => "trigger",
        OpenChangeReason::PressOutside => "press outside",
        OpenChangeReason::BackdropPress => "scrim",
        OpenChangeReason::Swipe => "swipe",
        OpenChangeReason::Cancel => "Escape",
        OpenChangeReason::Select => "choice",
        _ => "other",
    }
}

/// `Sheet` with detents: a bottom sheet on a phone (drag it down, press
/// the scrim, or Escape), a right side panel at desktop width.
pub(crate) fn sheet_section() -> impl IntoElement {
    let open = signal(false);
    let side = signal(Placement::Bottom);
    let detent = signal(1_u8);
    let push = signal(true);
    let mentions = signal(false);
    let status = signal("Closed".to_owned());
    view()
        .flex_col()
        .gap_3()
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(
                    overlay_button("Notification settings…", ButtonVariant::Outline, move || {
                        // The side is decided at open time from the window:
                        // bottom on a phone, a side panel at desktop width.
                        let next = if window().width.0 >= 600.0 {
                            Placement::Right
                        } else {
                            Placement::Bottom
                        };
                        side.set(next);
                        status.set(format!("Opened as a {} panel", side_name(next)));
                        open.set(true);
                    })                    ,
                )
                .child(overlay_button("Short", ButtonVariant::Ghost, move || detent.set(0)))
                .child(overlay_button("Half", ButtonVariant::Ghost, move || detent.set(1)))
                .child(overlay_button("Tall", ButtonVariant::Ghost, move || detent.set(2))),
        )
        .child(overlay_switch("Push notifications", push))
        .child(overlay_switch("Mentions only", mentions))
        .child(caption(
            "A side panel at desktop width, a bottom sheet with three detents and swipe-to-close on the phone; the tall detent reveals rows the shorter ones hide.",
        ))
        .child(caption(move || format!("Detent step {} of 0–2", detent.get())))
        .child(status_line({ let s = status; move || s.get() }))
        .child(sheet_host(
            open,
            side,
            detent,
            push,
            mentions,
            status,
        ))
}

/// A side's name for the status line.
fn side_name(side: Placement) -> &'static str {
    match side {
        Placement::Bottom => "bottom",
        Placement::Top => "top",
        Placement::Left => "left",
        Placement::Right => "right",
    }
}

/// The sheet itself, mounted only while open.
fn sheet_host(
    open: Signal<bool>,
    side: Signal<Placement>,
    detent: Signal<u8>,
    push: Signal<bool>,
    mentions: Signal<bool>,
    status: Signal<String>,
) -> impl IntoElement {
    when(open, move || {
        let side = side.get();
        let is_bottom = side == Placement::Bottom;
        sheet()
            // The sheet is controlled: the region mounts it while `open`
            // holds, and the component reads the same value.
            .open(move || Some(open.get()))
            .side(side)
            .popup_style(move |s| panel_look(side, detent, s))
            .backdrop_style(scrim)
            .close_on_swipe(is_bottom)
            .on_open_change(move |e: &SheetEvent| {
                if let SheetEvent::OpenChange(false, reason) = e {
                    // Controlled: a close request takes the value down.
                    status.set(format!("Closed ({})", reason_name(*reason)));
                    open.set(false);
                }
            })
            .child(
                view()
                    .flex_col()
                    .gap_3()
                    .child(
                        view()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .child(
                                text("Notification settings")
                                    .text_lg()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(move || theme().foreground),
                            )
                            .child(overlay_button("Done", ButtonVariant::Primary, move || {
                                status.set("Closed (Done)".to_owned());
                                open.set(false);
                            })),
                    )
                    .child(overlay_switch("Push notifications", push))
                    .child(overlay_switch("Mentions only", mentions))
                    .child(
                        text(move || format!("Detent step {}", detent.get()))
                            .text_xs()
                            .text_color(move || theme().muted_foreground),
                    )
                    .child(when(
                        move || detent.get() == 2,
                        || {
                            text("Extra rows fit at the tall detent")
                                .text_xs()
                                .test_id("sheet-tall-marker")
                                .text_color(move || theme().muted_foreground)
                        },
                    )),
            )
    })
}
