//! Ayah of the day (PARITY H2; web `(marketing)/daily-ayah`,
//! `lib/server/daily-ayah.ts`, the Yours `daily-ayah` widget). Contract:
//! `pub fn page() -> impl IntoElement`.
//!
//! The verse is `eq_core::home::daily_ayah::daily_verse` of today's UTC
//! date (the web's permutation, so both name the same verse); its Uthmani
//! text comes from the app's Quran source, verbatim.

use eq_core::{
    home::daily_ayah::{daily_verse, utc_date_key},
    i18n::keys,
    quran::{meta::meta, types::ArabicSource},
};
use nsh_base::theme;
use nsh_core::prelude::*;

use crate::{
    app::quran_store,
    features::bookmarks::{now_ms, surah_name},
    shell::{
        nav,
        routes::{ReaderLink, ReaderScope, ReaderTarget, Route},
    },
    stores::locale::{t, t_args},
    ui::{
        arabic::{ArabicStyle, QuranScript, plain_ayah},
        button::{ButtonSize, ButtonVariant, button_with},
        layout::{page_body, page_insets},
        text::{body, muted, page_heading},
        tokens,
    },
};

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) enum Daily {
    #[default]
    Loading,
    Unavailable,
    Ready {
        date: String,
        surah: u16,
        ayah: u16,
        text: String,
    },
}

/// Today's verse, loaded once per call.
pub(crate) fn daily() -> Signal<Daily> {
    let out = signal(Daily::Loading);
    let store = quran_store();
    let date = utc_date_key(now_ms());
    spawn(async move {
        let Some((global, surah, ayah)) = daily_verse(&date) else {
            let _ = out.try_set(Daily::Unavailable);
            return;
        };
        let next = match store
            .read_source(ArabicSource::Uthmani.id(), global, global)
            .await
        {
            Ok((range, _)) => match range.ayahs.into_iter().next() {
                Some(row) if !row.text.is_empty() => Daily::Ready {
                    date,
                    surah,
                    ayah,
                    text: row.text,
                },
                _ => Daily::Unavailable,
            },
            Err(e) => {
                log::warn!("daily ayah {surah}:{ayah}: {e}");
                Daily::Unavailable
            }
        };
        let _ = out.try_set(next);
    });
    out
}

/// The reader at the day's verse (Arabic).
pub(crate) fn open_in_reader(surah: u16, ayah: u16) {
    nav::push(Route::Reader(ReaderLink::new(ReaderTarget {
        scope: ReaderScope::Surah(surah),
        translation: None,
        ayah: Some(ayah),
    })));
}

fn arabic(verse: String, size: f32) -> impl IntoElement {
    plain_ayah(verse, None, move || ArabicStyle {
        size,
        ..ArabicStyle::of(QuranScript::Uthmani)
    })
    .text_color(tokens::quran_foreground)
}

/// The Yours / home widget body: name and key, the verse, Open.
pub(crate) fn daily_widget() -> impl IntoElement {
    let state = daily();
    dynamic(move || match state.get() {
        Daily::Loading => muted("…").into_element(),
        Daily::Unavailable => muted(t(keys::READER_WIDGET_DAILY_AYAH_UNAVAILABLE)).into_element(),
        Daily::Ready {
            surah,
            ayah,
            text: verse,
            ..
        } => view()
            .w_full()
            .flex_col()
            .gap_2()
            .px_4()
            .py_4()
            .rounded(tokens::radius_md)
            .border_1()
            .border_color(move || theme().border)
            .bg(tokens::surface)
            .child(
                view()
                    .flex_row()
                    .gap_1()
                    .child(
                        text(surah_name(i64::from(surah)))
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM),
                    )
                    .child(
                        text(format!("· {surah}:{ayah}"))
                            .text_sm()
                            .text_color(tokens::muted_text),
                    ),
            )
            .child(arabic(verse, 19.0))
            .child(view().flex_row().child(button_with(
                t(keys::READER_WIDGET_DAILY_AYAH_OPEN),
                ButtonVariant::Outline,
                ButtonSize::Small,
                false,
                None,
                move || open_in_reader(surah, ayah),
            )))
            .into_element(),
    })
}

pub fn page() -> impl IntoElement {
    let state = daily();
    scroll().flex_1().content_inset(page_insets(0.0)).child(
        page_body()
            .child(
                view()
                    .w_full()
                    .max_w(px(680.0))
                    .flex_col()
                    .gap_8()
                    .child(
                        view()
                            .flex_col()
                            .gap_3()
                            .child(page_heading(t(keys::DAILY_AYAH_HEADING)))
                            .child(body(t(keys::DAILY_AYAH_INTRO))),
                    )
                    .child(dynamic(move || match state.get() {
                        Daily::Loading => muted("…").into_element(),
                        Daily::Unavailable => body(t(keys::DAILY_AYAH_UNAVAILABLE)).into_element(),
                        Daily::Ready {
                            date,
                            surah,
                            ayah,
                            text: verse,
                        } => {
                            let name = meta()
                                .surah(surah)
                                .map_or_else(|| surah_name(i64::from(surah)), |s| s.name.clone());
                            view()
                                .w_full()
                                .flex_col()
                                .gap_8()
                                .child(
                                    view()
                                        .w_full()
                                        .flex_col()
                                        .gap_5()
                                        .p_6()
                                        .rounded(tokens::radius_md)
                                        .border_1()
                                        .border_color(move || theme().border)
                                        .bg(tokens::background_subtle)
                                        .child(arabic(verse, 28.0))
                                        .child(
                                            view()
                                                .w_full()
                                                .flex_row()
                                                .flex_wrap()
                                                .justify_between()
                                                .gap_3()
                                                .child(body(format!("{name} {surah}:{ayah}")))
                                                .child(body(t_args(
                                                    keys::DAILY_AYAH_DATE,
                                                    move || vec![("date", date.clone().into())],
                                                ))),
                                        ),
                                )
                                .child(view().flex_row().child(button_with(
                                    t(keys::DAILY_AYAH_OPEN),
                                    ButtonVariant::Ghost,
                                    ButtonSize::Small,
                                    false,
                                    None,
                                    move || open_in_reader(surah, ayah),
                                )))
                                .into_element()
                        }
                    })),
            )
            .text_color(move || theme().foreground),
    )
}
