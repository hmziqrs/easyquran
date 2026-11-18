//! The reader's bar (web `ReaderShell.svelte`'s sticky header):
//! translation slot, live position (`PositionIndicator`), the
//! Ayah-by-Ayah / Reading toggle (`ReaderModeToggle`), A−/A+ and the
//! reader settings sheet (script, face, size: the web's settings
//! "Reading" section, reachable from the reader on native).
//!
//! Seam: [`reader_bar`] takes `leading`, the translation button the
//! translations slice adds.

use eq_core::{
    i18n::keys,
    quran::{read::is_script_enabled, types::ArabicSource},
    routes::ReaderMode,
};
use nsh_core::element::AnyElement;
use nsh_core::prelude::*;

use super::model::position_of;
use crate::{
    stores::{
        locale::{t, t_args, tr},
        reader::{ReaderStore, data_environment},
    },
    ui::{
        arabic::ArabicFont,
        button::{ButtonSize, ButtonVariant, button_with, choice_button, icon_button},
        layout::divider,
        sheet::sheet_of,
        text::section_title,
        tokens,
    },
};

/// "Page 2 · Juz 1 · Hizb 1" for the topmost verse (`positionLabel`).
fn position_text(top: Signal<Option<(u16, u16)>>) -> impl Fn() -> SharedString + 'static {
    let page = t_args(keys::READER_POSITION_PAGE, move || {
        let p = top.get().and_then(|(s, a)| position_of(s, a));
        vec![("page", i64::from(p.map_or(1, |p| p.global_page)).into())]
    });
    let juz = t_args(keys::READER_POSITION_JUZ, move || {
        let p = top.get().and_then(|(s, a)| position_of(s, a));
        vec![("juz", i64::from(p.map_or(1, |p| p.juz)).into())]
    });
    let hizb = t_args(keys::READER_POSITION_HIZB, move || {
        let p = top.get().and_then(|(s, a)| position_of(s, a));
        vec![("hizb", i64::from(p.map_or(1, |p| p.hizb)).into())]
    });
    move || {
        if top.get().is_none() {
            return SharedString::default();
        }
        format!("{} · {} · {}", page(), juz(), hizb()).into()
    }
}

/// The mode switch; `change` keeps the reading position across the reflow.
fn mode_toggle(
    prefs: ReaderStore,
    change: impl Fn(ReaderMode) + Copy + 'static,
) -> impl IntoElement {
    view()
        .flex_row()
        .items_center()
        .gap_1()
        .label(tr(keys::READER_READING_MODE))
        .child(choice_button(
            t(keys::READER_AYAHS),
            tr(keys::READER_AYAH_BY_AYAH),
            move || prefs.mode() == ReaderMode::Verse,
            false,
            move || change(ReaderMode::Verse),
        ))
        .child(choice_button(
            t(keys::READER_READING),
            tr(keys::READER_READING),
            move || prefs.mode() == ReaderMode::Reading,
            false,
            move || change(ReaderMode::Reading),
        ))
}

/// The bar above the text.
pub fn reader_bar(
    prefs: ReaderStore,
    top: Signal<Option<(u16, u16)>>,
    settings_open: Signal<bool>,
    change_mode: impl Fn(ReaderMode) + Copy + 'static,
    leading: Option<AnyElement>,
) -> impl IntoElement {
    view()
        .w_full()
        .flex_row()
        .items_center()
        .gap_2()
        .px_4()
        .py_2()
        .border_b_1()
        .border_color(move || nsh_base::theme().border)
        .bg(move || nsh_base::theme().background)
        .child(leading)
        .child(
            text(position_text(top))
                .flex_1()
                .min_w(px(0.0))
                .number_of_lines(1)
                .text_xs()
                .tabular_nums()
                .text_color(tokens::muted_text),
        )
        .child(crate::features::audio::surah_action(move || {
            top.get().map(|(surah, _)| surah)
        }))
        .child(mode_toggle(prefs, change_mode))
        .child(
            view()
                .flex_row()
                .items_center()
                .label(tr(keys::READER_ARABIC_TEXT_SIZE))
                .child(
                    icon_button("A−", tr(keys::READER_SMALLER_ARABIC_TEXT), move || {
                        prefs.smaller();
                    })
                    .disabled(move || !prefs.can_shrink()),
                )
                .child(
                    icon_button("A+", tr(keys::READER_LARGER_ARABIC_TEXT), move || {
                        prefs.bigger();
                    })
                    .disabled(move || !prefs.can_grow()),
                ),
        )
        .child(icon_button("⋯", tr(keys::READER_SETTINGS), move || {
            settings_open.set(true);
        }))
}

fn script_label(source: ArabicSource) -> eq_core::i18n::Key {
    match source {
        ArabicSource::Uthmani => keys::SETTINGS_READING_SCRIPT_UTHMANI,
        ArabicSource::AnnotatedUthmani => keys::SETTINGS_READING_SCRIPT_UTHMANI_ANNOTATED,
        ArabicSource::SimpleClean => keys::SETTINGS_READING_SCRIPT_SIMPLE_CLEAN,
        ArabicSource::IndoPak => keys::SETTINGS_READING_SCRIPT_INDOPAK,
        ArabicSource::Tajweed => keys::SETTINGS_READING_SCRIPT_TAJWEED,
    }
}

fn font_label(font: ArabicFont) -> eq_core::i18n::Key {
    match font {
        ArabicFont::Amiri => keys::SETTINGS_READING_FONT_AMIRI,
        ArabicFont::ScheherazadeNew => keys::SETTINGS_READING_FONT_SCHEHERAZADE,
        ArabicFont::NotoNaskhArabic => keys::SETTINGS_READING_FONT_NOTO,
        ArabicFont::KfgqpcHafs => keys::SETTINGS_READING_FONT_HAFS,
        ArabicFont::KfgqpcTahaV1 => keys::SETTINGS_READING_FONT_TAHA_V1,
        ArabicFont::KfgqpcTahaV1Bold => keys::SETTINGS_READING_FONT_TAHA_V1_BOLD,
    }
}

fn group(title: eq_core::i18n::Key, items: Vec<AnyElement>) -> impl IntoElement {
    view()
        .gap_2()
        .child(section_title(t(title)))
        .child(view().flex_row().flex_wrap().gap_2().children(items))
}

/// The reader settings sheet: mode, script (the 5 sources; production
/// gates IndoPak and Tajweed as the web does), face, Arabic size.
pub fn settings_sheet(
    prefs: ReaderStore,
    open: Signal<bool>,
    change_mode: impl Fn(ReaderMode) + Copy + 'static,
) -> impl IntoElement {
    let side = if is_desktop() {
        Placement::Right
    } else {
        Placement::Bottom
    };
    sheet_of(open, side, move || {
        let env = data_environment();
        let scripts = ArabicSource::ALL
            .into_iter()
            .map(|s| {
                choice_button(
                    t(script_label(s)),
                    tr(script_label(s)),
                    move || prefs.script() == s,
                    !is_script_enabled(s, env),
                    move || prefs.set_script(s),
                )
                .into_element()
            })
            .collect();
        let fonts = ArabicFont::ALL
            .into_iter()
            .map(|f| {
                choice_button(
                    t(font_label(f)),
                    tr(font_label(f)),
                    move || prefs.arabic_font() == f,
                    move || prefs.script() == ArabicSource::IndoPak,
                    move || prefs.set_arabic_font(f),
                )
                .into_element()
            })
            .collect();
        scroll()
            .flex_1()
            .child(
                view()
                    .gap_4()
                    .pb_4()
                    .child(section_title(t(keys::READER_SETTINGS)))
                    .child(group(
                        keys::READER_READING_MODE,
                        vec![mode_toggle(prefs, change_mode).into_element()],
                    ))
                    .child(divider())
                    .child(group(keys::SETTINGS_READING_SCRIPT, scripts))
                    .child(group(keys::SETTINGS_READING_ARABIC_FONT, fonts))
                    .child(divider())
                    .child(group(
                        keys::READER_ARABIC_TEXT_SIZE,
                        vec![
                            button_with(
                                "A−",
                                ButtonVariant::Outline,
                                ButtonSize::Small,
                                move || !prefs.can_shrink(),
                                Some(tr(keys::READER_SMALLER_ARABIC_TEXT)),
                                move || prefs.smaller(),
                            )
                            .into_element(),
                            text(move || format!("{:.0}", prefs.font_size()))
                                .tabular_nums()
                                .text_sm()
                                .min_w(px(32.0))
                                .into_element(),
                            button_with(
                                "A+",
                                ButtonVariant::Outline,
                                ButtonSize::Small,
                                move || !prefs.can_grow(),
                                Some(tr(keys::READER_LARGER_ARABIC_TEXT)),
                                move || prefs.bigger(),
                            )
                            .into_element(),
                        ],
                    )),
            )
            .into_element()
    })
}
