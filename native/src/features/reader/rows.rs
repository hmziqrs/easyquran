//! The reader's rows (web `VerseRow.svelte`, `ReadingAyah.svelte`,
//! `IndoPakAyah.svelte`, the surah header of `ReaderHeader.svelte` and the
//! range readers' group bands). Every Quran string goes to the screen as
//! stored: tajweed runs and IndoPak word pieces are verbatim slices of the
//! row; ornaments and separators are spans beside the text, never in it.
//!
//! Seam for the verse-tools / word-view / translations slice:
//! [`verse_row`] takes `extra_lanes` (more elements under the lanes) and
//! `actions` (the row's tool strip).

use eq_core::i18n::keys;
use eq_core::quran::{
    meta::{Place, meta},
    types::QuranScript as Script,
    view::{
        VerseView,
        tajweed::{TajweedSegment, is_tajweed_script, parse_tajweed_segments},
    },
};
use nsh_core::LineHeight;
use nsh_core::element::{AnyElement, Text, View, WritingDirection};
use nsh_core::prelude::*;

use super::model::{LaneText, Opener};
use crate::{
    stores::{locale::t_args, reader::ReaderStore},
    ui::{
        arabic::{ArabicSegment, ArabicStyle, QuranScript, ayah_ornament, ayah_text},
        tokens::{self, TajweedClass},
    },
};

/// The app's text script of an edition (the style only tells IndoPak apart).
pub fn ui_script(script: Script) -> QuranScript {
    match script {
        Script::SimpleClean => QuranScript::SimpleClean,
        Script::IndoPak => QuranScript::IndoPak,
        Script::Tajweed => QuranScript::Tajweed,
        Script::Uthmani | Script::Translation => QuranScript::Uthmani,
    }
}

fn segment(s: TajweedSegment) -> ArabicSegment {
    let class = s.rule.and_then(|r| TajweedClass::from_letter(r.letter()));
    ArabicSegment {
        text: s.text.into(),
        class,
    }
}

/// Runs of verbatim text: tajweed runs, or the whole text as one run.
fn runs_of(text: &str, script: Script) -> Vec<ArabicSegment> {
    if is_tajweed_script(script) {
        return parse_tajweed_segments(text)
            .into_iter()
            .map(segment)
            .collect();
    }
    vec![ArabicSegment::plain(text.to_owned())]
}

/// The ornament's whitespace and sign spacing: kept in the text, drawn
/// zero-width (`.indopak-space { font-size: 0 }`).
fn zero_width(s: String) -> Text {
    text(s).text_size(px(0.01))
}

/// One verse's Arabic with its ayah-end ornament (`ReadingAyah`).
/// IndoPak follows `IndoPakAyah`: body words, the final word, the end
/// sign's marks (their source spaces drawn zero-width), then the IndoPak
/// medallion with Urdu digits after the sign.
pub fn arabic_of(verse: &VerseView, prefs: ReaderStore) -> Text {
    let script = ui_script(verse.script);
    let style = move || prefs.style_for(script);
    let Some(ending) = verse.indopak() else {
        return ayah_text(
            runs_of(&verse.body, verse.script),
            Some(u32::from(verse.ayah)),
            style,
        );
    };
    let mut paragraph = ayah_text(
        vec![
            ArabicSegment::plain(ending.body.clone()),
            ArabicSegment::plain(ending.last_word_text.clone()),
        ],
        None,
        style,
    );
    if !ending.last_word_space.is_empty() {
        paragraph = paragraph.child(zero_width(ending.last_word_space.clone()));
    }
    let annotated: String = ending
        .annotations
        .iter()
        .map(|a| format!("{}{}", a.mark, a.space))
        .collect();
    let tail = format!("{}{}", ending.sign, ending.following);
    if annotated == tail {
        for a in &ending.annotations {
            paragraph = paragraph.child(text(a.mark.clone()));
            if !a.space.is_empty() {
                paragraph = paragraph.child(zero_width(a.space.clone()));
            }
        }
    } else if !tail.is_empty() {
        paragraph = paragraph.child(text(tail));
    }
    paragraph.child(ayah_ornament(
        u32::from(verse.ayah),
        QuranScript::IndoPak,
        style,
    ))
}

/// A translation lane (`StackedLanes`): verbatim text in the reader's
/// translation size, its declared direction, 1.85 line height.
pub fn lane(lane: &LaneText, prefs: ReaderStore, labelled: bool) -> View {
    let rtl = lane.rtl;
    let size = move || {
        let base = prefs.translation_size();
        px(if rtl { base * 1.2 } else { base })
    };
    let line = if rtl { 2.05 } else { 1.85 };
    view()
        .gap_1()
        .when(labelled, |v| {
            v.child(
                text(lane.name.clone())
                    .text_xs()
                    .text_color(tokens::muted_text),
            )
        })
        .child(
            text(lane.text.clone())
                .writing_direction(if rtl {
                    WritingDirection::Rtl
                } else {
                    WritingDirection::Ltr
                })
                .text_size(size)
                .line_height(LineHeight::Multiple(line))
                .text_color(tokens::translation_foreground),
        )
}

/// One verse (Ayah-by-Ayah). `actions` is the row's tool strip (verse
/// tools slice), `extra_lanes` more content under the lanes (word view,
/// stacked-lane states).
pub fn verse_row(
    verse: &VerseView,
    lanes: &[LaneText],
    prefs: ReaderStore,
    actions: Option<AnyElement>,
    extra_lanes: Vec<AnyElement>,
) -> View {
    let labelled = lanes.len() > 1;
    view()
        .w_full()
        .px_4()
        .pt_4()
        .pb(px(26.0))
        .gap_3()
        .border_b_1()
        .border_color(tokens::reader_divider)
        .label(verse.key.clone())
        .child(
            view()
                .flex_row()
                .items_center()
                .justify_between()
                .min_h(px(24.0))
                .child(
                    text(verse.key.clone())
                        .text_xs()
                        .tabular_nums()
                        .text_color(tokens::muted_text),
                )
                .child(actions),
        )
        .child(arabic_of(verse, prefs).w_full())
        .children(lanes.iter().map(|l| lane(l, prefs, labelled)))
        .children(extra_lanes)
}

/// Reading: the verses of one page as one justified paragraph, each with
/// its ornament, a space between them.
pub fn flow_row(verses: &[VerseView], prefs: ReaderStore) -> View {
    let script = verses
        .first()
        .map_or(QuranScript::Uthmani, |v| ui_script(v.script));
    let style = move || prefs.style_for(script);
    let mut paragraph = text("")
        .writing_direction(WritingDirection::Rtl)
        .text_size(move || px(style().drawn_size()))
        .line_height(move || LineHeight::Multiple(style().line_height))
        .text_color(tokens::quran_foreground);
    for (i, verse) in verses.iter().enumerate() {
        if i > 0 {
            paragraph = paragraph.child(text(" "));
        }
        paragraph = paragraph.child(arabic_of(verse, prefs));
    }
    view().w_full().px_4().py_3().child(paragraph.w_full())
}

fn place_label(place: Place) -> eq_core::i18n::Key {
    match place {
        Place::Meccan => keys::SEARCH_NAV_PLACE_MECCAN,
        Place::Medinan => keys::SEARCH_NAV_PLACE_MEDINAN,
    }
}

/// The opener (bismillah) above verse 1, verbatim from the source's
/// normalization; tajweed keeps its colours.
fn opener_view(opener: &Opener, prefs: ReaderStore) -> View {
    let script = ui_script(opener.normalization.script);
    let style = move || ArabicStyle {
        ..prefs.style_for(script)
    };
    view().w_full().items_center().py_4().child(ayah_text(
        runs_of(&opener.raw, opener.normalization.script),
        None,
        style,
    ))
}

/// A surah's band (`ReaderHeader` strip): number, name, Arabic name,
/// "Meccan · 286 ayahs", then the opener when the run starts at verse 1.
pub fn surah_header(surah: u16, opener: Option<&Opener>, prefs: ReaderStore) -> View {
    let Some(s) = meta().surah(surah) else {
        return view();
    };
    let count = s.ayah_count;
    let place = place_label(s.place);
    let meta_line = t_args(keys::READER_INDEX_AYAH_COUNT, move || {
        vec![("count", i64::from(count).into())]
    });
    view()
        .w_full()
        .px_4()
        .pt_6()
        .gap_2()
        .border_b_1()
        .border_color(tokens::reader_divider)
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_3()
                .pb_4()
                .child(
                    view()
                        .size(px(36.0))
                        .rounded_full()
                        .items_center()
                        .justify_center()
                        .bg(move || nsh_base::theme().primary)
                        .child(
                            text(surah.to_string())
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(move || nsh_base::theme().primary_foreground),
                        ),
                )
                .child(
                    text(s.name.clone())
                        .text_xl()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(move || nsh_base::theme().foreground),
                )
                .child(
                    text(s.arabic.clone())
                        .writing_direction(WritingDirection::Rtl)
                        .text_2xl()
                        .text_color(move || nsh_base::theme().foreground),
                )
                .child(
                    text(move || format!("{} · {}", crate::stores::locale::tr(place), meta_line()))
                        .text_sm()
                        .text_color(tokens::foreground_secondary),
                ),
        )
        .child(opener.map(|o| opener_view(o, prefs)))
}
