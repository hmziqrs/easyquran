//! Toasts: short messages over the app, with a swipe to dismiss. One
//! host per surface ([`ToastHost`]); anything calls [`notify`].

use std::time::Duration;

use nsh_base::{
    Anchor, ToastManager, ToastMotion, ToastOptions, ToastStackState, theme, toast, toast_stack,
};
use nsh_core::{prelude::*, time::now};

/// The queued messages, drained by the host; a global so a listener or a
/// task anywhere can notify without context.
fn queue() -> Signal<Vec<SharedString>> {
    global::<Vec<SharedString>>()
}

/// A message for a while over the app: "Copied", "Could not send".
pub fn notify(message: impl Into<SharedString>) {
    queue().update(|q| q.push(message.into()));
}

/// The host that draws the queue as toasts: mounted once, app level.
#[derive(Clone, PartialEq, Props)]
pub struct ToastHostProps {}

/// The toast stack and its lifecycle, driven on the engine clock.
#[component]
pub fn ToastHost(_props: ToastHostProps) -> impl IntoElement {
    let manager = stored(ToastManager::<u64, SharedString>::new(ToastMotion::sonner()));
    let shown = signal(Vec::new());
    let state = ToastStackState::new();
    let next_id = global::<u64>();
    let sync = move || {
        shown.set(manager.with(|m| {
            m.visible(3)
                .map(|(id, message, status)| (*id, message.clone(), status))
                .collect::<Vec<_>>()
        }))
    };
    // The lifecycle advances on the engine clock, paused while expanded.
    set_interval(Duration::from_millis(50), move || {
        let drained = queue().with(Vec::clone);
        if !drained.is_empty() {
            queue().set(Vec::new());
            for message in drained {
                next_id.update(|n| *n += 1);
                let id = next_id.get_untracked();
                manager.with(|m| {
                    m.push(
                        id,
                        message,
                        ToastOptions::timeout(Some(Duration::from_secs(5))),
                        now(),
                    )
                });
            }
            sync();
        }
        if manager.with(|m| m.advance(now(), state.is_expanded()).changed) {
            sync();
        }
    });
    toast_stack()
        .placement(Anchor::BottomCenter)
        .state(state)
        .w(px(340.0))
        .child(each(
            shown,
            |(id, _, _)| *id,
            move |item, _| {
                toast()
                    .transition_status(move || item.get().2)
                    .accessibility_label(move || Some(item.get().1.clone()))
                    .on_dismiss_request(move || {
                        manager.with(|m| m.dismiss(&item.get().0, now()));
                    })
                    .flex_col()
                    .rounded_xl()
                    .px_4()
                    .py_3()
                    .bg(move || theme().card)
                    .border_1()
                    .border_color(move || theme().border)
                    .shadow_lg()
                    .transition_all()
                    .duration(200.0)
                    // While swiped, no transition: the toast tracks the
                    // pointer, not an eased value behind it.
                    .swiping_class("transition-none")
                    .starting_class("opacity-0 translate-y-4")
                    .ending_class("opacity-0 scale-95")
                    .child(
                        text(move || item.get().1.clone())
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(move || theme().card_foreground),
                    )
                    .child(
                        text("Swipe to dismiss")
                            .text_xs()
                            .text_color(move || theme().muted_foreground),
                    )
            },
        ))
}
