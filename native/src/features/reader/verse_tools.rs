//! The verse tool strip (PARITY C9): web `VerseTools.svelte` and
//! `ReportVerseDialog.svelte` + `report/payload.ts`. Copy, share, the
//! range share dialog, and (where a translation is read) a correction
//! report. Bookmark, pin, reading-bookmark and note come from the
//! bookmarks feature through the seams below.

use std::time::Duration;

use eq_core::{api::types::ReportBody, i18n::keys, quran::view::VerseView, routes::TranslationRef};
use nsh_core::element::AnyElement;
use nsh_core::prelude::*;

use super::share;
use crate::{
    stores::{
        locale::{t, t_args, tr},
        reader::ReaderStore,
    },
    ui::{
        button::{ButtonSize, ButtonVariant, button_with, icon_svg_button},
        dialog::dialog_card,
        field::{multiline_field, text_field},
        icons::lucide,
        tokens,
    },
};

const COPY: &str = lucide!(
    r#"<rect width="14" height="14" x="8" y="8" rx="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/>"#
);
const CHECK: &str = lucide!(r#"<path d="M20 6 9 17l-5-5"/>"#);
const SHARE: &str = lucide!(
    r#"<circle cx="18" cy="5" r="3"/><circle cx="6" cy="12" r="3"/><circle cx="18" cy="19" r="3"/><path d="m8.59 13.51 6.83 3.98"/><path d="m15.41 6.51-6.82 3.98"/>"#
);
const SHARE_OPTIONS: &str = lucide!(
    r#"<rect width="18" height="18" x="3" y="3" rx="2"/><circle cx="9" cy="9" r="2"/><path d="m21 15-3.09-3.09a2 2 0 0 0-2.82 0L6 21"/>"#
);
const FLAG: &str = lucide!(
    r#"<path d="M4 15s1-1 4-1 5 2 8 2 4-1 4-1V3s-1 1-4 1-5-2-8-2-4 1-4 1z"/><path d="M4 22v-7"/>"#
);

/// `COPY_FEEDBACK_MS`.
const COPY_FEEDBACK: Duration = Duration::from_millis(1500);

fn bookmark_action(surah: u16, ayah: u16) -> Option<AnyElement> {
    Some(crate::features::bookmarks::bookmark_toggle(surah, ayah).into_element())
}

fn note_action(surah: u16, ayah: u16) -> Option<AnyElement> {
    Some(crate::features::bookmarks::note_editor(surah, ayah).into_element())
}

// wired by lead after B1 merges (pins + "bookmark here" live with bookmarks)
fn pin_action(_surah: u16, _ayah: u16) -> Option<AnyElement> {
    None
}

// wired by lead after B1 merges
fn reading_bookmark_action(_surah: u16, _ayah: u16) -> Option<AnyElement> {
    None
}

/// The tool strip of one verse row. `route` is the translation being read
/// (none on the Arabic reader).
pub fn verse_tools(
    verse: &VerseView,
    route: Option<TranslationRef>,
    prefs: ReaderStore,
) -> AnyElement {
    let (surah, ayah) = (verse.surah, verse.ayah);
    let body = std::rc::Rc::new(verse.body.clone());
    let share_open = signal(false);
    let report_open = signal(false);
    let copy = {
        let (route, body) = (route.clone(), body.clone());
        std::rc::Rc::new(move || share::copy_verse(route.as_ref(), surah, ayah, &body))
    };
    let share_text: std::rc::Rc<dyn Fn()> = {
        let (route, body) = (route.clone(), body.clone());
        std::rc::Rc::new(move || share::share_verse(route.as_ref(), surah, ayah, &body))
    };
    let script = untracked(|| prefs.script());
    view()
        .flex_row()
        .items_center()
        .gap(px(2.0))
        .child(pin_action(surah, ayah))
        .child(bookmark_action(surah, ayah))
        .child(reading_bookmark_action(surah, ayah))
        .child(shared_flash(COPY, tr(keys::READER_COPY), copy))
        .child(shared_flash(SHARE, tr(keys::READER_SHARE), share_text))
        .child(icon_svg_button(
            SHARE_OPTIONS,
            tr(keys::READER_SHARE_OPTIONS),
            move || share_open.set(true),
        ))
        .child(route.is_some().then(|| {
            icon_svg_button(FLAG, tr(keys::READER_REPORT_VERSE), move || {
                report_open.set(true);
            })
        }))
        .child(note_action(surah, ayah))
        .child(share::share_dialog(
            share_open,
            surah,
            ayah,
            route.clone(),
            script,
        ))
        .child(crate::features::audio::verse_actions(surah, ayah))
        .child(route.map(|r| report_dialog(report_open, r.id(), format!("{surah}:{ayah}"))))
        .into_element()
}

/// An icon action that shows a check for a moment after it ran.
fn shared_flash(
    icon: &'static str,
    name: &'static str,
    run: std::rc::Rc<dyn Fn()>,
) -> impl IntoElement {
    let done = signal(false);
    dynamic(move || {
        let shown = if done.get() { CHECK } else { icon };
        let run = run.clone();
        untracked(move || {
            icon_svg_button(shown, name, move || {
                run();
                done.set(true);
                spawn(async move {
                    timer(COPY_FEEDBACK).await;
                    done.set(false);
                });
            })
            .into_element()
        })
    })
}

// ---------------------------------------------------------------------
// Report (`payload.ts`, `ReportVerseDialog.svelte`)
// ---------------------------------------------------------------------

/// `REPORT_DETAILS_MAX` (UTF-16 units, as the web counts).
pub const REPORT_DETAILS_MAX: usize = 2000;

/// Why a report body could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportError {
    InvalidTarget,
    DetailsTooLong,
}

fn positive_int(s: &str) -> Option<u32> {
    let ok = !s.is_empty() && !s.starts_with('0') && s.bytes().all(|b| b.is_ascii_digit());
    if !ok {
        return None;
    }
    s.parse().ok()
}

/// `^[a-z]{2,3}\.[a-z0-9_-]{1,40}$`.
fn is_source_id(s: &str) -> bool {
    let Some((lang, rest)) = s.split_once('.') else {
        return false;
    };
    (2..=3).contains(&lang.len())
        && lang.bytes().all(|b| b.is_ascii_lowercase())
        && (1..=40).contains(&rest.len())
        && rest
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

/// `isTranslationTarget`: `<source_id>:<surah 1-114>:<ayah 1-286>`.
pub fn is_translation_target(raw: &str) -> bool {
    let parts: Vec<&str> = raw.split(':').collect();
    let [source, surah, ayah] = parts.as_slice() else {
        return false;
    };
    let (Some(s), Some(a)) = (positive_int(surah), positive_int(ayah)) else {
        return false;
    };
    is_source_id(source) && s <= 114 && a <= 286
}

/// `buildReportPayload` for a translation report (reason `other`).
pub fn build_report(
    target_id: &str,
    details: &str,
    contact: &str,
) -> Result<ReportBody, ReportError> {
    let target = target_id.trim();
    if !is_translation_target(target) {
        return Err(ReportError::InvalidTarget);
    }
    let details = Some(details.trim()).filter(|d| !d.is_empty());
    if details.is_some_and(|d| d.encode_utf16().count() > REPORT_DETAILS_MAX) {
        return Err(ReportError::DetailsTooLong);
    }
    let contact = Some(contact.trim())
        .filter(|c| !c.is_empty())
        .map(str::to_lowercase);
    Ok(ReportBody {
        target_kind: "translation".to_owned(),
        target_id: target.to_owned(),
        reason: "other".to_owned(),
        details: details.map(str::to_owned),
        contact_email: contact,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Idle,
    Invalid,
    TooLong,
    Sending,
    Sent,
    Error,
}

fn status_message(status: Status) -> SharedString {
    match status {
        Status::Sent => tr(keys::READER_REPORT_SENT).into(),
        Status::Error | Status::Invalid => tr(keys::READER_REPORT_FAILED).into(),
        Status::TooLong => {
            let max = i64::try_from(REPORT_DETAILS_MAX).unwrap_or(0);
            t_args(keys::READER_REPORT_TOO_LONG, move || {
                vec![("max", max.into())]
            })()
        }
        Status::Idle | Status::Sending => SharedString::default(),
    }
}

/// The report dialog for verse `verse_key` read in `source_id`.
fn report_dialog(open: Signal<bool>, source_id: String, verse_key: String) -> impl IntoElement {
    let target = format!("{source_id}:{verse_key}");
    let details = signal(String::new());
    let contact = signal(String::new());
    let status = signal(Status::Idle);
    let submit = std::rc::Rc::new(move || {
        let now = status.get_untracked();
        if now == Status::Sending || now == Status::Sent {
            return;
        }
        let body = match build_report(&target, &details.get_untracked(), &contact.get_untracked()) {
            Ok(body) => body,
            Err(ReportError::DetailsTooLong) => {
                status.set(Status::TooLong);
                return;
            }
            Err(ReportError::InvalidTarget) => {
                status.set(Status::Invalid);
                return;
            }
        };
        status.set(Status::Sending);
        let client = crate::app::auth_store().client();
        spawn(async move {
            let Some(client) = client else {
                status.set(Status::Error);
                return;
            };
            let sent = client.report_submit(&body).await.is_ok();
            status.set(if sent { Status::Sent } else { Status::Error });
        });
    });
    dialog_card(
        open,
        tr(keys::READER_REPORT_TITLE).into(),
        move || open.set(false),
        move || {
            view()
                .gap_3()
                .child(
                    text(t(keys::READER_REPORT_DESCRIPTION))
                        .text_sm()
                        .text_color(tokens::foreground_secondary),
                )
                .child(
                    text(t(keys::READER_REPORT_DETAILS))
                        .text_xs()
                        .text_color(tokens::foreground_secondary),
                )
                .child(multiline_field(details, ""))
                .child(
                    text(t(keys::READER_REPORT_CONTACT))
                        .text_xs()
                        .text_color(tokens::foreground_secondary),
                )
                .child(text_field(contact, ""))
                .child(
                    text(move || status_message(status.get()))
                        .text_xs()
                        .text_color(tokens::muted_text),
                )
                .into_element()
        },
        move || {
            view()
                .flex_row()
                .justify_end()
                .gap_2()
                .child(button_with(
                    t(keys::READER_REPORT_CANCEL),
                    ButtonVariant::Ghost,
                    ButtonSize::Small,
                    false,
                    None,
                    move || open.set(false),
                ))
                .child(button_with(
                    move || {
                        if status.get() == Status::Sending {
                            return SharedString::from(tr(keys::READER_REPORT_SENDING));
                        }
                        SharedString::from(tr(keys::READER_REPORT_SEND))
                    },
                    ButtonVariant::Primary,
                    ButtonSize::Small,
                    move || matches!(status.get(), Status::Sending | Status::Sent),
                    None,
                    {
                        let submit = submit.clone();
                        move || submit()
                    },
                ))
                .into_element()
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_payload_matches_web_rules() {
        assert!(is_translation_target("en.sahih:2:255"));
        assert!(!is_translation_target("quranenc.en.x:2:255"));
        assert!(!is_translation_target("en.sahih:115:1"));
        assert!(!is_translation_target("en.sahih:02:1"));
        let body = build_report(" en.sahih:1:1 ", "  ", " A@B.C ").expect("valid");
        assert_eq!(body.details, None);
        assert_eq!(body.contact_email.as_deref(), Some("a@b.c"));
        let long = "x".repeat(REPORT_DETAILS_MAX + 1);
        assert!(matches!(
            build_report("en.sahih:1:1", &long, ""),
            Err(ReportError::DetailsTooLong)
        ));
    }
}
