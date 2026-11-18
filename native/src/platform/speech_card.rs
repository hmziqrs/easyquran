//! The speech card: dictation through `recognize_speech`, with its
//! availability shown. Reading text aloud is not exposed on 0.5.0 and the
//! card says so.

use nsh_base::theme;
use nsh_core::{prelude::*, speech::SpeechRequest};

// nsh_core 0.5.0's prelude has a `t!` of its own; the app translates with rust-i18n.
use rust_i18n::t;

use super::{
    locale,
    widgets::{action_button, hint, panel},
};

#[component]
fn SpeechCard() -> impl IntoElement {
    // Availability is answered once, on show; it does not change mid-run.
    let available = signal(None::<bool>);
    let listening = signal(false);
    let transcript = signal(String::new());
    let owner = Owner::current();
    spawn(async move {
        let ok = speech_available().await;
        if owner.is_live() {
            available.set(Some(ok));
        }
    });
    panel(
        "platform.speech.title",
        [
            // One line for both states, so the availability also translates.
            text(move || {
                locale::app_locale().get();
                let key = match available.get() {
                    Some(true) => "platform.speech.available",
                    Some(false) => "platform.speech.unavailable",
                    None => "platform.speech.checking",
                };
                t!(key).to_string()
            })
            .text_sm()
            .text_color(move || {
                if available.get() == Some(true) {
                    theme().foreground
                } else {
                    theme().muted_foreground
                }
            })
            .into_element(),
            action_button(
                "platform-speech-start",
                move || {
                    locale::app_locale().get();
                    if listening.get() {
                        t!("platform.speech.listening").to_string()
                    } else {
                        t!("platform.speech.start").to_string()
                    }
                },
                move || {
                    if listening.get_untracked() {
                        return;
                    }
                    listening.set(true);
                    transcript.set(String::new());
                    let owner = Owner::current();
                    let mut session = recognize_speech(SpeechRequest::default());
                    spawn(async move {
                        while let Some(event) = session.next().await {
                            if !owner.is_live() {
                                break;
                            }
                            if let nsh_core::speech::SpeechEvent::Result { text, .. } = event {
                                transcript.set(text);
                            }
                        }
                        if owner.is_live() {
                            listening.set(false);
                        }
                    });
                },
            )
            .into_element(),
            // The transcript so far; the placeholder translates.
            text(move || {
                locale::app_locale().get();
                let heard = transcript.get();
                if heard.is_empty() {
                    t!("platform.speech.transcript_empty").to_string()
                } else {
                    heard
                }
            })
            .text_sm()
            .test_id("platform-speech-transcript")
            .into_element(),
            hint("platform.speech.note"),
        ],
    )
}

/// The speech card, as the platform page places it.
pub(crate) fn card() -> AnyElement {
    view().child(speech_card()).into_element()
}
