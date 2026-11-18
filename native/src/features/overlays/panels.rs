//! Resizable panels: a `ResizablePanelGroup` whose handles drag and whose
//! sizes are data; the sizes also move by the keyboard through the
//! section's step buttons, the way an assistive technology's increment and
//! decrement do.

use super::{ButtonVariant, caption, overlay_button, status_line};
use nsh_base::{h_resizable, resizable_panel, theme};
use nsh_core::prelude::*;

/// The lane a panel renders: a titled card, so the split reads.
fn lane(title: &'static str, note: &'static str) -> impl IntoElement {
    view()
        .flex_1()
        .min_h_0()
        .flex_col()
        .gap_1()
        .p_4()
        .rounded(move || theme().radius)
        .bg(move || theme().muted.opacity(0.35))
        .overflow_hidden()
        .child(
            text(title)
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(move || theme().foreground),
        )
        .child(
            text(note)
                .text_xs()
                .text_color(move || theme().muted_foreground),
        )
}

/// Resizable panels with keyboard-resizable handles.
pub(crate) fn panels_section() -> impl IntoElement {
    // The sizes the group reports and the buttons write: one value, so a
    // drag and the keyboard agree.
    let sizes = signal(vec![px(200.0), px(0.0)]);
    let status = signal("Drag the divider, or step it by 10 pt.".to_owned());
    view()
        .flex_col()
        .gap_3()
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .child(
                    overlay_button("Wider", ButtonVariant::Outline, {
                                                move || {
                            sizes.update(|s| {
                                s[0] = px((s[0].0 + 40.0).clamp(120.0, 560.0));
                            });
                            status.set(format!("First panel at {} pt", sizes.get()[0].0 as i32));
                        }
                    })                    ,
                )
                .child(
                    overlay_button("Narrower", ButtonVariant::Outline, {
                                                move || {
                            sizes.update(|s| {
                                s[0] = px((s[0].0 - 40.0).clamp(120.0, 560.0));
                            });
                            status.set(format!("First panel at {} pt", sizes.get()[0].0 as i32));
                        }
                    })                    ,
                ),
        )
        .child(
            h_resizable()
                .h(px(260.0))
                .handle_accessibility_label("Resize the panels")
                .handle_style(|s| {
                    s.w(px(5.0))
                        .rounded_full()
                        .bg(move || theme().border)
                        .transition_colors()
                })
                .handle_thumb_style(|s| {
                    s.h(px(24.0))
                        .w(px(3.0))
                        .rounded_full()
                        .bg(move || theme().muted_foreground)
                        .opacity(0.5)
                })
                .on_resize({
                                                            move |next: &Vec<Points>| {
                        if let Some(first) = next.first() {
                            sizes.update(|s| s[0] = *first);
                        }
                        let widths: Vec<String> = next
                            .iter()
                            .map(|size| format!("{}", size.0 as i32))
                            .collect();
                        status.set(format!("Resized to [{}] pt", widths.join(", ")));
                    }
                })
                .child(
                    resizable_panel()
                        .size(move || Some(sizes.get()[0]))
                        .min_size(px(120.0))
                        .max_size(px(560.0))
                        .child(lane("Explorer", "The files of the workspace.")),
                )
                .child(
                    resizable_panel()
                        .child(lane("Viewer", "Takes the rest of the row.")),
                ),
        )
        .child(caption(
            "Handles are adjustable for screen readers: one increment or decrement moves a panel 10 points.",
        ))
        .child(status_line({ let s = status; move || s.get() }))
}
