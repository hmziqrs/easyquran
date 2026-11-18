//! Reading (PARITY J3; web `ReadingSection.svelte`): mode, Arabic script
//! (production gates IndoPak and Tajweed), Arabic font, Arabic size,
//! translation font and size, instant resume. UI over `ReaderStore` (and
//! `AppearanceStore` for instant resume, which the web keeps in prefs).
//!
//! Not here: the web's live preview, which sets fixed specimen verses;
//! Quran text on native only comes from the databases.

use eq_core::{
    i18n::{Key, keys},
    quran::{read::is_script_enabled, types::ArabicSource},
    routes::ReaderMode,
};
use nsh_core::{element::AnyElement, prelude::*};

use super::{pill, section_frame};
use crate::{
    features::downloads::rows::{caption, list_card, row_frame, row_label},
    shell::routes::SettingsSection,
    stores::{
        appearance::AppearanceStore,
        locale::{t, tr},
        reader::{
            ReaderStore, TRANSLATION_FONT_MAX, TRANSLATION_FONT_MIN, TranslationFamily,
            data_environment,
        },
    },
    ui::{
        arabic::ArabicFont,
        button::{ButtonSize, ButtonVariant, button_with},
        text::{TextRole, role},
        tokens,
    },
};

fn script_key(source: ArabicSource) -> Key {
    match source {
        ArabicSource::Uthmani => keys::SETTINGS_READING_SCRIPT_UTHMANI,
        ArabicSource::AnnotatedUthmani => keys::SETTINGS_READING_SCRIPT_UTHMANI_ANNOTATED,
        ArabicSource::SimpleClean => keys::SETTINGS_READING_SCRIPT_SIMPLE_CLEAN,
        ArabicSource::IndoPak => keys::SETTINGS_READING_SCRIPT_INDOPAK,
        ArabicSource::Tajweed => keys::SETTINGS_READING_SCRIPT_TAJWEED,
    }
}

fn font_key(font: ArabicFont) -> Key {
    match font {
        ArabicFont::Amiri => keys::SETTINGS_READING_FONT_AMIRI,
        ArabicFont::ScheherazadeNew => keys::SETTINGS_READING_FONT_SCHEHERAZADE,
        ArabicFont::NotoNaskhArabic => keys::SETTINGS_READING_FONT_NOTO,
        ArabicFont::KfgqpcHafs => keys::SETTINGS_READING_FONT_HAFS,
        ArabicFont::KfgqpcTahaV1 => keys::SETTINGS_READING_FONT_TAHA_V1,
        ArabicFont::KfgqpcTahaV1Bold => keys::SETTINGS_READING_FONT_TAHA_V1_BOLD,
    }
}

fn family_key(family: TranslationFamily) -> Key {
    match family {
        TranslationFamily::Sans => keys::SETTINGS_READING_FONT_SANS,
        TranslationFamily::Serif => keys::SETTINGS_READING_FONT_SERIF,
    }
}

fn choices(title: Key, items: Vec<AnyElement>, note: Option<AnyElement>) -> AnyElement {
    row_frame()
        .child(row_label(t(title)))
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_1p5()
                .max_w(px(448.0))
                .children(items),
        )
        .children(note)
        .into_element()
}

fn stepper(
    title: Key,
    value: impl Fn() -> f32 + Copy + 'static,
    can_shrink: impl Fn() -> bool + 'static,
    can_grow: impl Fn() -> bool + 'static,
    shrink: impl Fn() + 'static,
    grow: impl Fn() + 'static,
) -> AnyElement {
    row_frame()
        .child(row_label(t(title)))
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_2p5()
                .label(tr(title))
                .child(button_with(
                    "−",
                    ButtonVariant::Outline,
                    ButtonSize::Small,
                    move || !can_shrink(),
                    Some(tr(title)),
                    shrink,
                ))
                .child(
                    role(
                        move || SharedString::from(format!("{}px", value())),
                        TextRole::Caption,
                    )
                    .tabular_nums()
                    .min_w(px(48.0))
                    .text_center()
                    .text_color(tokens::foreground_secondary),
                )
                .child(button_with(
                    "+",
                    ButtonVariant::Outline,
                    ButtonSize::Small,
                    move || !can_grow(),
                    Some(tr(title)),
                    grow,
                )),
        )
        .into_element()
}

fn resume_row() -> AnyElement {
    let prefs = AppearanceStore::get();
    let label = move || {
        let key = if prefs.instant_resume() {
            keys::READER_ON
        } else {
            keys::READER_OFF
        };
        SharedString::from(tr(key))
    };
    row_frame()
        .flex_row()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap_4()
        .child(
            view()
                .flex_1()
                .min_w_0()
                .flex_col()
                .gap_0p5()
                .child(row_label(t(keys::SETTINGS_READING_RESUME)))
                .child(caption(t(keys::SETTINGS_READING_RESUME_NOTE))),
        )
        .child(pill(
            label,
            tr(keys::SETTINGS_READING_RESUME),
            move || prefs.instant_resume(),
            move || prefs.set_instant_resume(!prefs.instant_resume()),
        ))
        .into_element()
}

pub fn section() -> AnyElement {
    let reader = ReaderStore::get();
    let env = data_environment();
    let modes = [
        (ReaderMode::Verse, keys::READER_AYAH_BY_AYAH),
        (ReaderMode::Reading, keys::READER_READING),
    ]
    .into_iter()
    .map(|(m, key)| {
        pill(
            t(key),
            tr(key),
            move || reader.mode() == m,
            move || reader.set_mode(m),
        )
    })
    .collect();
    let scripts = ArabicSource::ALL
        .into_iter()
        .map(|s| {
            crate::ui::button::choice_button(
                t(script_key(s)),
                tr(script_key(s)),
                move || reader.script() == s,
                !is_script_enabled(s, env),
                move || reader.set_script(s),
            )
            .into_element()
        })
        .collect();
    let disabled_note = ArabicSource::ALL
        .into_iter()
        .any(|s| !is_script_enabled(s, env))
        .then(|| caption(t(keys::SETTINGS_READING_SCRIPT_DISABLED_NOTE)).into_element());
    let fonts = ArabicFont::ALL
        .into_iter()
        .map(|f| {
            pill(
                t(font_key(f)),
                tr(font_key(f)),
                move || reader.arabic_font() == f,
                move || reader.set_arabic_font(f),
            )
        })
        .collect();
    let families = [TranslationFamily::Sans, TranslationFamily::Serif]
        .into_iter()
        .map(|f| {
            pill(
                t(family_key(f)),
                tr(family_key(f)),
                move || reader.translation_family() == f,
                move || reader.set_translation_family(f),
            )
        })
        .collect();
    section_frame(
        SettingsSection::Reading,
        keys::SETTINGS_READING_INTRO,
        list_card(vec![
            choices(keys::READER_MODE, modes, None),
            choices(keys::SETTINGS_READING_SCRIPT, scripts, disabled_note),
            choices(keys::SETTINGS_READING_ARABIC_FONT, fonts, None),
            stepper(
                keys::READER_ARABIC_TEXT_SIZE,
                move || reader.font_size(),
                move || reader.can_shrink(),
                move || reader.can_grow(),
                move || reader.smaller(),
                move || reader.bigger(),
            ),
            choices(keys::SETTINGS_READING_TRANSLATION_FONT, families, None),
            stepper(
                keys::SETTINGS_READING_TRANSLATION_SIZE,
                move || reader.translation_size(),
                move || reader.translation_size() > TRANSLATION_FONT_MIN,
                move || reader.translation_size() < TRANSLATION_FONT_MAX,
                move || reader.shrink_translation(),
                move || reader.grow_translation(),
            ),
            resume_row(),
        ]),
    )
}
