//! Result rendering (`ResultSection.svelte`, `AyahResult.svelte`): a
//! section per source with its count, rows of verbatim text with the
//! matched spans lit, load more, retry, and the gated notes. The two
//! highlighted-text builders are shared with the command palette.

use eq_core::{
    i18n::keys,
    quran::{
        catalogue::{self, Direction},
        meta::meta,
    },
    routes::{ReaderLink, ReaderScope, ReaderTarget, TranslationRef},
    search::{Highlight, HitKind, SearchHit, highlights::highlight_segments},
};
use nsh_base::theme;
use nsh_core::{LineHeight, element::WritingDirection, prelude::*};

use super::engine::{Engine, Phase, Section};
use crate::{
    shell::{nav, routes::Route},
    stores::{
        locale::{t, t_args},
        reader::ReaderStore,
    },
    ui::{
        arabic::QuranScript,
        button::{ButtonSize, ButtonVariant, button_with},
        layout::{pressable_card, skeleton_of},
        text::{muted, section_title},
        tokens,
    },
};

/// A verse's text with the matched spans lit, set as the Quran is
/// (right to left, the reader's face). The bytes go out verbatim; the
/// spans only cut them.
pub(crate) fn highlighted_quran(text: String, highlights: &[Highlight], size: f32) -> Text {
    let style = ReaderStore::get().style_for(QuranScript::Uthmani);
    let mut paragraph = text_frame(writing(true))
        .font_family(SharedString::from(style.family()))
        .font_weight(style.font.weight())
        .text_size(px(size))
        .line_height(LineHeight::Multiple(1.9))
        .text_color(tokens::quran_foreground);
    for (piece, lit) in pieces(&text, highlights) {
        paragraph = paragraph.child(span(piece, lit));
    }
    paragraph
}

/// A translation's text with the matched spans lit, in its direction.
pub(crate) fn highlighted_text(text: String, highlights: &[Highlight], rtl: bool) -> Text {
    let mut paragraph = text_frame(writing(rtl))
        .text_sm()
        .text_color(tokens::translation_foreground);
    for (piece, lit) in pieces(&text, highlights) {
        paragraph = paragraph.child(span(piece, lit));
    }
    paragraph
}

fn writing(rtl: bool) -> WritingDirection {
    if rtl {
        WritingDirection::Rtl
    } else {
        WritingDirection::Ltr
    }
}

fn text_frame(direction: WritingDirection) -> Text {
    text("").writing_direction(direction)
}

fn pieces(text: &str, highlights: &[Highlight]) -> Vec<(String, bool)> {
    highlight_segments(text, highlights)
        .into_iter()
        .map(|s| (s.text(text).to_owned(), s.highlighted))
        .collect()
}

fn span(piece: String, lit: bool) -> Text {
    let run = text(piece);
    if !lit {
        return run;
    }
    run.text_color(move || theme().primary)
        .bg(tokens::gold_soft)
}

/// Whether translation `id` reads right to left.
pub(crate) fn is_rtl(id: &str) -> bool {
    catalogue::translation(id).is_some_and(|t| t.direction == Direction::Rtl)
}

/// The reader at `surah:ayah`, in the section's source.
pub(crate) fn verse_link(section: &str, surah: u16, ayah: u16) -> Option<ReaderLink> {
    meta().surah_local_page_for_ayah(surah, ayah)?;
    let translation = (section != super::engine::ARABIC).then(|| TranslationRef::from_id(section));
    Some(ReaderLink::new(ReaderTarget {
        scope: ReaderScope::Surah(surah),
        translation,
        ayah: Some(ayah),
    }))
}

fn open(section: &str, surah: u16, ayah: u16) {
    if let Some(link) = verse_link(section, surah, ayah) {
        nav::open(Route::Reader(link));
    }
}

fn heading(section: &Section) -> SharedString {
    if section.is_arabic() {
        return SharedString::from(crate::stores::locale::tr(keys::SEARCH_SECTION_QURAN));
    }
    match catalogue::peek_translation_name(&section.id) {
        Some(name) => SharedString::from(name),
        None => SharedString::from(section.id.clone()),
    }
}

fn hit_label(hit: &SearchHit) -> String {
    let name = meta()
        .surah(hit.surah)
        .map(|s| s.name.clone())
        .unwrap_or_default();
    if hit.kind == HitKind::Opener {
        return name;
    }
    format!("{name} {}:{}", hit.surah, hit.ayah)
}

fn hit_row(section_id: String, hit: SearchHit) -> AnyElement {
    let label = hit_label(&hit);
    let (surah, ayah) = (hit.surah, hit.ayah);
    let arabic = section_id == super::engine::ARABIC;
    let rtl = arabic || is_rtl(&section_id);
    let body = if arabic {
        highlighted_quran(hit.text.clone(), &hit.highlights, 26.0)
    } else {
        highlighted_text(hit.text.clone(), &hit.highlights, rtl)
    };
    let id = section_id.clone();
    pressable_card(label.clone(), move || open(&id, surah, ayah))
        .gap_2()
        .child(
            text(label)
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(tokens::muted_text),
        )
        .child(body.w_full())
        .into_element()
}

fn count_line(total: usize) -> impl IntoElement {
    muted(t_args(keys::SEARCH_COUNT, move || {
        vec![("count", (total as i64).into())]
    }))
}

/// One section: heading and count, then its state.
pub(super) fn section_view(engine: Engine, id: String) -> AnyElement {
    let current = move || {
        engine
            .sections
            .with(|l| l.iter().find(|s| s.id == id).cloned())
    };
    view()
        .flex_col()
        .gap_3()
        .child(dynamic(move || {
            let Some(section) = current() else {
                return view().into_element();
            };
            section_body(engine, section)
        }))
        .into_element()
}

fn section_body(engine: Engine, section: Section) -> AnyElement {
    let header = view()
        .flex_row()
        .items_center()
        .justify_between()
        .gap_3()
        .child(section_title(heading(&section)))
        .child(
            (section.phase == Phase::Done || section.phase == Phase::More)
                .then(|| count_line(section.total)),
        );
    let mut body = view().flex_col().gap_3().child(header);
    match section.phase {
        Phase::Loading => {
            for _ in 0..3 {
                body = body.child(skeleton_of(64.0));
            }
        }
        Phase::Gated => {
            body = body.child(muted(t(keys::SEARCH_ARABIC_ONLY_NOTE)));
        }
        Phase::Error => {
            let id = section.id.clone();
            body = body.child(rows(&section)).child(
                view()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .child(muted(t(keys::SEARCH_ERROR_SECTION)))
                    .child(button_with(
                        t(keys::SEARCH_RETRY),
                        ButtonVariant::Outline,
                        ButtonSize::Small,
                        false,
                        None,
                        move || engine.retry(&id),
                    )),
            );
        }
        Phase::Done | Phase::More => {
            if section.hits.is_empty() {
                body = body.child(muted(t(keys::SEARCH_NO_RESULTS)));
            } else {
                body = body.child(rows(&section));
            }
            body = body.child(more_button(engine, &section));
        }
    }
    body.into_element()
}

fn rows(section: &Section) -> View {
    view().flex_col().gap_2().children(
        section
            .hits
            .iter()
            .cloned()
            .map(|hit| hit_row(section.id.clone(), hit)),
    )
}

fn more_button(engine: Engine, section: &Section) -> Option<AnyElement> {
    let loading = section.phase == Phase::More;
    if !section.can_load_more() && !loading {
        return None;
    }
    let id = section.id.clone();
    let label = if loading {
        keys::SEARCH_LOADING_MORE
    } else {
        keys::SEARCH_LOAD_MORE
    };
    Some(
        button_with(
            t(label),
            ButtonVariant::Outline,
            ButtonSize::Small,
            loading,
            None,
            move || engine.load_more(&id),
        )
        .self_start()
        .into_element(),
    )
}
