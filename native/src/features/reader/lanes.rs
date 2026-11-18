//! Translation lanes under a verse (PARITY C7): web `StackedLanes.svelte`
//! and `StackedLaneSettings.svelte`. One accent edge per lane, the text in
//! its declared direction, the credit after it; commentary (tafsir) as a
//! muted block; a lane that failed to read says so; side by side on wide
//! windows when the layout asks for it. Text is verbatim from the source.

use eq_core::{
    i18n::keys,
    quran::{catalogue, view::text_marks::parse_transliteration_segments},
};
use nsh_core::LineHeight;
use nsh_core::element::{AnyElement, WritingDirection};
use nsh_core::prelude::*;

use super::model::LaneText;
use crate::{
    stores::{
        locale::{t, t_args, tr},
        reader::{ReaderStore, TranslationFamily},
        translations::{LANE_STEP_MAX, LANE_STEP_MIN, LaneLayout, StackStore},
    },
    ui::{
        button::{ButtonSize, ButtonVariant, button_with, choice_button},
        tokens,
    },
};

/// Side-by-side lanes need room (`min-width: 768px`).
const WIDE: f32 = 768.0;

/// A lane row: its text, or the failure note.
enum Lane {
    Text(LaneText),
    Error(String),
}

impl Lane {
    fn source_id(&self) -> &str {
        match self {
            Lane::Text(l) => &l.source_id,
            Lane::Error(id) => id,
        }
    }
}

/// `laneCredit`: the translator, then the work when named differently.
fn lane_credit(id: &str) -> String {
    let Some(t) = catalogue::translation(id) else {
        return id.to_owned();
    };
    let who = t.translator.clone().unwrap_or_else(|| t.name.clone());
    if t.name.trim().to_lowercase() == who.trim().to_lowercase() {
        return who;
    }
    format!("{who} ({})", t.name)
}

fn family(prefs: ReaderStore, rtl: bool) -> SharedString {
    if rtl {
        return "Noto Naskh Arabic".into();
    }
    match prefs.translation_family() {
        TranslationFamily::Sans => "Onest, sans-serif".into(),
        TranslationFamily::Serif => "serif".into(),
    }
}

fn direction(rtl: bool) -> WritingDirection {
    if rtl {
        return WritingDirection::Rtl;
    }
    WritingDirection::Ltr
}

/// The lane's body: verbatim text, its transliteration marks drawn as
/// underline / bold spans, or the empty-verse note.
fn lane_body(lane: &LaneText, size: impl Fn() -> f32 + 'static, prefs: ReaderStore) -> AnyElement {
    let rtl = lane.rtl;
    let line = if rtl { 2.05 } else { 1.75 };
    if lane.text.trim().is_empty() {
        let who = catalogue::translation(&lane.source_id)
            .map_or_else(|| lane.name.clone(), |t| t.name.clone());
        return text(t_args(keys::READER_TRANSLATION_EMPTY_VERSE, move || {
            vec![("translator", who.clone().into())]
        }))
        .italic()
        .text_size(move || px(size() * 0.9))
        .text_color(tokens::muted_text)
        .into_element();
    }
    let mut body = text("")
        .writing_direction(direction(rtl))
        .text_size(move || px(size()))
        .line_height(LineHeight::Multiple(line))
        .font_family(move || family(prefs, rtl))
        .text_color(tokens::translation_foreground);
    for segment in parse_transliteration_segments(&lane.text) {
        let mut span = text(segment.text);
        if segment.underline {
            span = span.underline();
        }
        if segment.strong {
            span = span.font_weight(FontWeight::BOLD);
        }
        body = body.child(span);
    }
    body.into_element()
}

fn lane_view(lane: &Lane, prefs: ReaderStore, stack: StackStore) -> AnyElement {
    let id = lane.source_id().to_owned();
    let scale_id = id.clone();
    let rtl = match lane {
        Lane::Text(l) => l.rtl,
        Lane::Error(_) => false,
    };
    let size = move || {
        let base = prefs.translation_size() * stack.lane_scale(&scale_id);
        if rtl { base * 1.2 } else { base }
    };
    let tafsir = catalogue::translation(&id).is_some_and(|t| t.is_tafsir());
    let frame = view()
        .gap_1()
        .border_s(px(2.0))
        .border_s_color(move || {
            if tafsir {
                tokens::foreground_secondary()
            } else {
                nsh_base::theme().primary
            }
        })
        .pl(px(14.0))
        .pr(px(10.0))
        .when(tafsir, |v| v.bg(tokens::background_subtle).py_2());
    match lane {
        Lane::Error(_) => frame
            .child(
                text(t(keys::READER_STACKED_ERROR))
                    .text_sm()
                    .text_color(tokens::muted_text),
            )
            .into_element(),
        Lane::Text(l) => {
            let credit = if tafsir {
                format!(
                    "— {} ({})",
                    lane_credit(&id),
                    tr(keys::READER_COMMENTARY_LABEL)
                )
            } else {
                format!("— {}", lane_credit(&id))
            };
            frame
                .child(lane_body(l, size, prefs))
                .child(
                    text(credit)
                        .writing_direction(direction(rtl))
                        .text_size(px(13.0))
                        .text_color(tokens::foreground_secondary),
                )
                .into_element()
        }
    }
}

/// The lanes of one verse, in route order (`order`: the route's own
/// translation first, then the stack); `failed` lanes show their note.
pub fn stacked_lanes(
    lanes: &[LaneText],
    failed: &[String],
    order: &[String],
    prefs: ReaderStore,
) -> AnyElement {
    let stack = StackStore::get();
    let mut rows: Vec<Lane> = lanes.iter().cloned().map(Lane::Text).collect();
    rows.extend(
        failed
            .iter()
            .filter(|id| !lanes.iter().any(|l| l.source_id == **id))
            .cloned()
            .map(Lane::Error),
    );
    let rank = |id: &str| order.iter().position(|o| o == id).unwrap_or(order.len());
    rows.sort_by_key(|r| rank(r.source_id()));
    if rows.is_empty() {
        return view().into_element();
    }
    let rows = std::rc::Rc::new(rows);
    dynamic(move || {
        let parallel =
            stack.layout() == LaneLayout::Parallel && window().width.0 >= WIDE && rows.len() > 1;
        let rows = rows.clone();
        untracked(move || {
            let lanes: Vec<AnyElement> = rows
                .iter()
                .map(|r| {
                    let v = lane_view(r, prefs, stack);
                    if parallel {
                        return view().flex_1().min_w(px(0.0)).child(v).into_element();
                    }
                    v
                })
                .collect();
            if parallel {
                return view()
                    .w_full()
                    .flex_row()
                    .items_start()
                    .gap(px(20.0))
                    .children(lanes)
                    .into_element();
            }
            view().w_full().gap(px(10.0)).children(lanes).into_element()
        })
    })
    .into_element()
}

/// Lane display controls (`StackedLaneSettings`): stacked or side by side,
/// a text size per lane, and the reader's translation size and face.
pub fn lane_settings(stack: StackStore, prefs: ReaderStore) -> AnyElement {
    let layout_choice = |layout: LaneLayout, key| {
        choice_button(
            t(key),
            tr(key),
            move || stack.layout() == layout,
            false,
            move || stack.set_layout(layout),
        )
    };
    let lane_rows: Vec<AnyElement> = stack
        .ids()
        .into_iter()
        .map(|id| {
            let name = catalogue::translation(&id).map_or_else(|| id.clone(), |t| t.name.clone());
            let (a, b, c) = (id.clone(), id.clone(), id.clone());
            let (d, e, f) = (id.clone(), id.clone(), id);
            view()
                .flex_row()
                .items_center()
                .justify_between()
                .gap_3()
                .child(
                    text(name)
                        .flex_1()
                        .min_w(px(0.0))
                        .number_of_lines(1)
                        .text_sm()
                        .text_color(move || nsh_base::theme().foreground),
                )
                .child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap_1()
                        .child(button_with(
                            "A−",
                            ButtonVariant::Ghost,
                            ButtonSize::Small,
                            move || stack.lane_step(&a) <= LANE_STEP_MIN,
                            None,
                            move || stack.step_lane(&d, -1),
                        ))
                        .child(button_with(
                            "A",
                            ButtonVariant::Ghost,
                            ButtonSize::Small,
                            move || stack.lane_step(&b) == 0,
                            None,
                            move || stack.reset_lane(&e),
                        ))
                        .child(button_with(
                            "A+",
                            ButtonVariant::Ghost,
                            ButtonSize::Small,
                            move || stack.lane_step(&c) >= LANE_STEP_MAX,
                            None,
                            move || stack.step_lane(&f, 1),
                        )),
                )
                .into_element()
        })
        .collect();
    let family_choice = |family: TranslationFamily, key| {
        choice_button(
            t(key),
            tr(key),
            move || prefs.translation_family() == family,
            false,
            move || prefs.set_translation_family(family),
        )
    };
    let row = |label: eq_core::i18n::Key, controls: Vec<AnyElement>| {
        view()
            .flex_row()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_3()
            .child(
                text(t(label))
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(move || nsh_base::theme().foreground),
            )
            .child(view().flex_row().items_center().gap_1().children(controls))
    };
    view()
        .gap_3()
        .child(
            text(t(keys::READER_LANES_SETTINGS_TITLE))
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(tokens::muted_text),
        )
        .child(row(
            keys::READER_LANES_LAYOUT_LABEL,
            vec![
                layout_choice(LaneLayout::Stacked, keys::READER_LANES_LAYOUT_STACKED)
                    .into_element(),
                layout_choice(LaneLayout::Parallel, keys::READER_LANES_LAYOUT_PARALLEL)
                    .into_element(),
            ],
        ))
        .child(view().gap_1().children(lane_rows))
        .child(row(
            keys::SETTINGS_READING_TRANSLATION_SIZE,
            vec![
                button_with(
                    "A−",
                    ButtonVariant::Outline,
                    ButtonSize::Small,
                    move || prefs.translation_size() <= crate::stores::reader::TRANSLATION_FONT_MIN,
                    None,
                    move || prefs.shrink_translation(),
                )
                .into_element(),
                text(move || format!("{:.0}", prefs.translation_size()))
                    .tabular_nums()
                    .text_sm()
                    .min_w(px(28.0))
                    .into_element(),
                button_with(
                    "A+",
                    ButtonVariant::Outline,
                    ButtonSize::Small,
                    move || prefs.translation_size() >= crate::stores::reader::TRANSLATION_FONT_MAX,
                    None,
                    move || prefs.grow_translation(),
                )
                .into_element(),
            ],
        ))
        .child(row(
            keys::SETTINGS_READING_TRANSLATION_FONT,
            vec![
                family_choice(TranslationFamily::Sans, keys::SETTINGS_READING_FONT_SANS)
                    .into_element(),
                family_choice(TranslationFamily::Serif, keys::SETTINGS_READING_FONT_SERIF)
                    .into_element(),
            ],
        ))
        .into_element()
}
