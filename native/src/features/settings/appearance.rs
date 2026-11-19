//! Appearance (PARITY J2; web `AppearanceSection.svelte`): interface
//! language, appearance mode, palette, the three custom seed colours,
//! Copy CSS and Reset. UI over `AppearanceStore` and the locale store.

use eq_core::i18n::{Key, Locale, keys};
use nsh_base::theme;
use nsh_core::{element::AnyElement, prelude::*};

use super::{pill, section_frame};
use crate::{
    features::downloads::rows::{caption, list_card, row_frame, row_label},
    shell::routes::SettingsSection,
    stores::{
        appearance::{AppearanceMode, AppearanceStore, SeedKey},
        locale::{self, t, tr},
    },
    ui::{
        button::{ButtonSize, ButtonVariant, button_with},
        color::color_picker_of,
        popups::popover_of,
        text::{TextRole, role},
        toast::notify,
        tokens::{self, Palette, derive},
    },
};

fn mode_key(mode: AppearanceMode) -> Key {
    match mode {
        AppearanceMode::Light => keys::THEME_LIGHT,
        AppearanceMode::Dark => keys::THEME_DARK,
        AppearanceMode::System => keys::THEME_SYSTEM,
    }
}

fn palette_keys(palette: Palette) -> (Key, Key) {
    match palette {
        Palette::Sacred => (
            keys::SETTINGS_PALETTE_SACRED_LABEL,
            keys::SETTINGS_PALETTE_SACRED_NOTE,
        ),
        Palette::Ink => (
            keys::SETTINGS_PALETTE_INK_LABEL,
            keys::SETTINGS_PALETTE_INK_NOTE,
        ),
        Palette::Sepia => (
            keys::SETTINGS_PALETTE_SEPIA_LABEL,
            keys::SETTINGS_PALETTE_SEPIA_NOTE,
        ),
        Palette::Sapphire => (
            keys::SETTINGS_PALETTE_SAPPHIRE_LABEL,
            keys::SETTINGS_PALETTE_SAPPHIRE_NOTE,
        ),
    }
}

fn seed_key(key: SeedKey) -> Key {
    match key {
        SeedKey::Bg => keys::READER_BACKGROUND,
        SeedKey::Accent => keys::READER_ACCENT,
        SeedKey::Pop => keys::READER_POP,
    }
}

/// The color a seed falls back to (the palette's own token).
fn seed_fallback(key: SeedKey) -> Hsla {
    let color = match key {
        SeedKey::Bg => theme().background,
        SeedKey::Accent => theme().primary,
        SeedKey::Pop => theme().accent,
    };
    tokens::resolved(color)
}

fn to_hex(color: Hsla) -> String {
    let rgba = color.to_rgb();
    derive::to_hex(derive::Rgb {
        r: f64::from(rgba.r) * 255.0,
        g: f64::from(rgba.g) * 255.0,
        b: f64::from(rgba.b) * 255.0,
    })
}

fn row(label: Key, control: impl IntoElement) -> AnyElement {
    row_frame()
        .flex_row()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap_4()
        .child(row_label(t(label)))
        .child(control)
        .into_element()
}

fn language_row() -> AnyElement {
    let pills = view()
        .flex_row()
        .gap_1p5()
        .children(Locale::ALL.into_iter().map(|l| {
            pill(
                SharedString::from(l.endonym()),
                l.endonym(),
                move || locale::locale() == l,
                move || locale::set_locale(l),
            )
        }));
    row(keys::SETTINGS_LANGUAGE, pills)
}

fn mode_row(store: AppearanceStore) -> AnyElement {
    let pills = view()
        .flex_row()
        .gap_1p5()
        .children(AppearanceMode::ALL.into_iter().map(|m| {
            pill(
                t(mode_key(m)),
                tr(mode_key(m)),
                move || store.mode() == m,
                move || store.set_mode(m),
            )
        }));
    row(keys::SETTINGS_SECTION_APPEARANCE, pills)
}

fn palette_row(store: AppearanceStore) -> AnyElement {
    let options = Palette::ALL.into_iter().map(move |p| {
        let (label, note) = palette_keys(p);
        nsh_base::button()
            .on_press(move || store.set_palette(p))
            .accessibility_label(tr(label))
            .selected(move || store.palette() == p)
            .flex_row()
            .items_center()
            .gap_3()
            .px_3()
            .py(px(10.0))
            .min_w(px(220.0))
            .flex_1()
            .rounded(tokens::radius_pill)
            .border_1()
            .border_color(tokens::border_strong)
            .bg(tokens::surface)
            .hover(|s| s.border_color(move || theme().border))
            .focus(crate::ui::button::focus_ring)
            .selected_style(|s| {
                s.border_color(move || theme().primary)
                    .bg(tokens::primary_soft)
            })
            .child(
                view()
                    .size(px(24.0))
                    .flex_shrink_0()
                    .rounded(tokens::radius_sm)
                    .border_1()
                    .border_color(tokens::border_strong)
                    .bg(move || p.swatch()),
            )
            .child(
                view()
                    .flex_1()
                    .min_w_0()
                    .flex_col()
                    .child(role(t(label), TextRole::Caption))
                    .child(caption(t(note)).number_of_lines(1)),
            )
            .into_element()
    });
    row_frame()
        .child(row_label(t(keys::SETTINGS_PALETTE)))
        .child(view().flex_row().flex_wrap().gap_2().children(options))
        .into_element()
}

fn seed_row(store: AppearanceStore, key: SeedKey) -> AnyElement {
    let value = signal(seed_fallback(key));
    let current = move || store.prefs().seed(key).map(str::to_owned);
    let swatch = move || match current().as_deref().and_then(derive::parse_hex) {
        Some(rgb) => AdaptiveColor::from(rgb.to_hsla()),
        None => AdaptiveColor::from(seed_fallback(key)),
    };
    let trigger = nsh_base::button()
        .accessibility_label(tr(keys::TWEAKS_COLOUR_INPUT))
        .size(px(32.0))
        .flex_shrink_0()
        .rounded(tokens::radius_sm)
        .border_1()
        .border_color(tokens::border_strong)
        .focus(crate::ui::button::focus_ring)
        .bg(swatch);
    let picker = view()
        .w(px(240.0))
        .p_3()
        .child(color_picker_of(value, move |c| {
            store.set_custom(key, Some(&to_hex(c)));
        }));
    let name = t(seed_key(key));
    view()
        .flex_row()
        .items_center()
        .gap_3()
        .child(popover_of(trigger, picker, move |open| {
            if open {
                let start = current()
                    .as_deref()
                    .and_then(derive::parse_hex)
                    .map_or_else(|| seed_fallback(key), derive::Rgb::to_hsla);
                value.set(start);
            }
        }))
        .child(
            role(name, TextRole::Caption)
                .flex_1()
                .text_color(tokens::foreground_secondary),
        )
        .child(
            caption(move || {
                SharedString::from(current().unwrap_or_else(|| tr(keys::TWEAKS_PRESET).to_owned()))
            })
            .tabular_nums(),
        )
        .child(when(
            move || current().is_some(),
            move || {
                crate::ui::button::icon_button("✕", tr(keys::TWEAKS_RESET_TO_PRESET), move || {
                    store.set_custom(key, None);
                })
                .into_element()
            },
        ))
        .into_element()
}

fn custom_row(store: AppearanceStore) -> AnyElement {
    row_frame()
        .child(
            view()
                .flex_row()
                .items_center()
                .justify_between()
                .gap_4()
                .child(row_label(t(keys::READER_CUSTOM_COLOURS)))
                .child(when(
                    move || store.has_custom(),
                    move || {
                        button_with(
                            t(keys::READER_CLEAR),
                            ButtonVariant::Ghost,
                            ButtonSize::Small,
                            false,
                            None,
                            move || store.clear_custom(),
                        )
                        .into_element()
                    },
                )),
        )
        .child(
            view()
                .flex_col()
                .gap_2()
                .children(SeedKey::ALL.into_iter().map(move |k| seed_row(store, k))),
        )
        .child(caption(t(keys::READER_THEME_DERIVED_NOTE)))
        .into_element()
}

pub fn section() -> AnyElement {
    let store = AppearanceStore::get();
    let copied = signal(false);
    let copy_label = move || {
        if copied.get() {
            return SharedString::from(tr(keys::READER_COPIED));
        }
        SharedString::from(tr(keys::READER_COPY_CSS))
    };
    let actions = view()
        .flex_row()
        .flex_wrap()
        .gap_2()
        .child(
            button_with(
                copy_label,
                ButtonVariant::Outline,
                ButtonSize::Medium,
                false,
                None,
                move || {
                    set_clipboard(store.css());
                    copied.set(true);
                    notify(tr(keys::READER_COPIED));
                    set_timeout(std::time::Duration::from_millis(1600), move || {
                        copied.set(false);
                    });
                },
            )
            .flex_1(),
        )
        .child(
            button_with(
                t(keys::READER_RESET),
                ButtonVariant::Outline,
                ButtonSize::Medium,
                false,
                None,
                move || store.reset(),
            )
            .flex_1(),
        );
    section_frame(
        SettingsSection::Appearance,
        keys::SETTINGS_APPEARANCE_INTRO,
        view()
            .w_full()
            .flex_col()
            .gap_4()
            .child(list_card(vec![
                language_row(),
                mode_row(store),
                palette_row(store),
                custom_row(store),
            ]))
            .child(actions),
    )
}
