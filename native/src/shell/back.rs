//! The Android back order, in one place: the engine asks the newest modal
//! first (the palette, this prompt), then the root's listener pops pushed
//! pages, and only with nothing pushed does it ask before letting the
//! platform exit the app.

use nsh_base::{dialog, theme};
use nsh_core::{element::AnyElement, prelude::*};

use crate::{
    shell::{chrome, nav},
    ui::button::{ButtonSize, ButtonVariant, button_with},
};

/// What the shell's back press does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackStep {
    /// The top pushed page pops.
    PopPage,
    /// Nothing is pushed: the app asks before the platform exits it.
    AskExit,
    /// The ask was confirmed once: the next back is the platform's own,
    /// which exits the app (the listener declines it).
    Exit,
}

/// The decision for a stack `depth` with `exit_armed` set.
pub fn back_step(depth: usize, exit_armed: bool) -> BackStep {
    if depth > 0 {
        BackStep::PopPage
    } else if exit_armed {
        BackStep::Exit
    } else {
        BackStep::AskExit
    }
}

/// The exit prompt is up.
#[derive(Default, Clone, PartialEq)]
pub struct PromptOpen(pub bool);

/// The exit ask is armed: the next back with nothing pushed exits.
#[derive(Default, Clone, PartialEq)]
pub struct ExitArmed(pub bool);

fn prompt() -> Signal<PromptOpen> {
    global::<PromptOpen>()
}

fn arm(on: bool) {
    global::<ExitArmed>().set(ExitArmed(on));
}

/// The root's back listener: owns every press no modal took. Returning
/// false hands the press to the platform's own back, which exits the app
/// once nothing is pushed.
pub fn install() {
    on_back_press(
        move || match back_step(nav::depth(), global::<ExitArmed>().get_untracked().0) {
            BackStep::PopPage => {
                nav::pop();
                arm(false);
                true
            }
            BackStep::AskExit => {
                prompt().set(PromptOpen(true));
                true
            }
            BackStep::Exit => false,
        },
    );
}

/// The exit ask, over everything while it is open.
pub fn exit_prompt_dialog() -> AnyElement {
    let open = prompt();
    when(
        move || open.with(|open| open.0),
        || {
            // Wrapper gap: ui::dialog_of lays out a full page, so the
            // centered card over a scrim keeps nsh_base::dialog here.
            dialog()
                .close_on_escape(false)
                .on_cancel(move || prompt().set(PromptOpen(false)))
                .flex_1()
                .flex_col()
                .items_center()
                .justify_center()
                .bg(move || theme().foreground.opacity(0.25))
                .child(
                    view()
                        .w(px(280.0))
                        .items_center()
                        .gap_2()
                        .p_4()
                        .rounded_lg()
                        .bg(move || theme().popover)
                        .border_1()
                        .border_color(move || theme().border)
                        .child(
                            text("Exit nsh-starter?")
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(move || theme().popover_foreground),
                        )
                        .child(
                            text("This closes the app.")
                                .text_xs()
                                .text_color(move || theme().muted_foreground),
                        )
                        .child(
                            view()
                                .flex_row()
                                .justify_end()
                                .gap_2()
                                .child(button_with(
                                    "Cancel",
                                    ButtonVariant::Ghost,
                                    ButtonSize::Medium,
                                    false,
                                    None,
                                    move || prompt().set(PromptOpen(false)),
                                ))
                                .child(button_with(
                                    "Exit",
                                    ButtonVariant::Danger,
                                    ButtonSize::Medium,
                                    false,
                                    None,
                                    exit,
                                )),
                        ),
                )
                .into_element()
        },
    )
    .into_element()
}

/// The ask was confirmed: arm the platform's exit, close the prompt and,
/// on the desktops, close the window as the same command.
fn exit() {
    arm(true);
    prompt().set(PromptOpen(false));
    chrome::close();
}
