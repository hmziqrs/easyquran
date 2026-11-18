//! Voice search (PARITY E5; `search/voice/recognition.ts`,
//! `VoiceSearchButton.svelte`) on NSH's speech recognition
//! (`recognize_speech`; the `speech` capability). Shown only where the
//! platform recognizes speech; the final transcript goes to the query as
//! typed text would. Arabic UI listens as `ar-SA`, English as `en-US`.

use std::{cell::RefCell, rc::Rc};

use eq_core::i18n::{Locale, keys};
use nsh_core::{
    prelude::*,
    speech::{SpeechError, SpeechEvent, SpeechRequest},
};

use crate::{
    stores::locale::{locale, t},
    ui::{button::icon_svg_button, text::muted},
};

const MIC: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z"/><path d="M19 10v2a7 7 0 0 1-14 0v-2"/><line x1="12" x2="12" y1="19" y2="22"/></svg>"#;

/// Where a listening session is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Idle,
    Listening,
    Denied,
    NoSpeech,
    Failed,
}

/// `voiceLang`.
fn voice_lang(locale: Locale) -> &'static str {
    match locale {
        Locale::Ar => "ar-SA",
        Locale::En => "en-US",
    }
}

/// `failureFor`: `None` for a session the app or user stopped.
fn failure_for(error: SpeechError) -> Status {
    match error {
        SpeechError::NotAllowed => Status::Denied,
        SpeechError::NoSpeech => Status::NoSpeech,
        _ => Status::Failed,
    }
}

fn status_key(status: Status) -> Option<eq_core::i18n::Key> {
    match status {
        Status::Idle => None,
        Status::Listening => Some(keys::SEARCH_VOICE_LISTENING),
        Status::Denied => Some(keys::SEARCH_VOICE_DENIED),
        Status::NoSpeech => Some(keys::SEARCH_VOICE_NO_SPEECH),
        Status::Failed => Some(keys::SEARCH_VOICE_FAILED),
    }
}

type Session = Rc<RefCell<Option<nsh_core::element::SpeechRecognition>>>;

fn start(
    status: Signal<Status>,
    session: Session,
    generation: Signal<u64>,
    on: Rc<dyn Fn(String)>,
) {
    // Events from a replaced session must not touch the current one.
    if let Some(old) = session.borrow_mut().take() {
        old.abort();
    }
    generation.update(|g| *g += 1);
    let mine = generation.get_untracked();
    let mut request = SpeechRequest::default();
    request.lang = Some(voice_lang(locale()).to_owned());
    request.interim = false;
    *session.borrow_mut() = Some(recognize_speech(request));
    status.set(Status::Listening);
    spawn(async move {
        loop {
            let event = std::future::poll_fn(|cx| match session.borrow_mut().as_mut() {
                Some(s) => s.poll_next(cx),
                None => std::task::Poll::Ready(None),
            })
            .await;
            if generation.get_untracked() != mine {
                return;
            }
            match event {
                Some(SpeechEvent::Result { text, is_final }) => {
                    let text = text.trim().to_owned();
                    if is_final && !text.is_empty() {
                        status.set(Status::Idle);
                        on(text);
                    }
                }
                Some(SpeechEvent::End { error }) => {
                    let next = match error {
                        Some(e) => failure_for(e),
                        None if status.get_untracked() == Status::Listening => Status::NoSpeech,
                        None => status.get_untracked(),
                    };
                    status.set(next);
                    session.borrow_mut().take();
                    return;
                }
                Some(_) => {}
                None => {
                    session.borrow_mut().take();
                    return;
                }
            }
        }
    });
}

/// The microphone button and its status line, or nothing where speech is
/// not recognized. `on_transcript` gets the final transcript.
pub(super) fn voice_button(on_transcript: impl Fn(String) + 'static) -> AnyElement {
    let available = signal(false);
    spawn(async move {
        available.set(speech_available().await);
    });
    let status = signal(Status::Idle);
    let generation = signal(0u64);
    let session: Session = Rc::new(RefCell::new(None));
    let on: Rc<dyn Fn(String)> = Rc::new(on_transcript);
    cleanup({
        let session = session.clone();
        move || {
            if let Some(s) = session.borrow_mut().take() {
                s.abort();
            }
        }
    });
    when(
        move || available.get(),
        move || {
            let session = session.clone();
            let on = on.clone();
            let toggle = move || {
                if status.get_untracked() == Status::Listening {
                    if let Some(s) = session.borrow().as_ref() {
                        s.stop();
                    }
                    return;
                }
                start(status, session.clone(), generation, on.clone());
            };
            view()
                .flex_row()
                .items_center()
                .gap_2()
                // i18n: search.voice_start / search.voice_stop
                .child(icon_svg_button(MIC, "Search by voice", toggle))
                .child(dynamic(move || {
                    status_key(status.get()).map(|key| muted(t(key)).into_element())
                }))
                .into_element()
        },
    )
    .into_element()
}
