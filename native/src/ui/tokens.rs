//! The app's design tokens, mirrored from the web's `routes/layout.css`
//! contract (§4): four palettes (sacred, ink, sepia, sapphire), each light
//! and dark, over one shared neutral ground, plus the user's custom seeds
//! (`lib/theme/derive.ts`).
//!
//! Where the values live: `themes/theme.json` is the default palette
//! (sacred) in full, the file `nsh.json` hands the engine at launch;
//! `themes/palettes/<id>.json` hold what each palette changes (the
//! accent family). [`theme_json`] composes the three layers (base,
//! palette, derived seeds) into one theme file and [`tokens_for`] reads it
//! with `nsh_base`'s own reader, so `stores::appearance` can switch the
//! whole app at run time with `nsh_core::element::set_tokens` (an engine
//! action: every binding that read `theme()` follows natively).
//!
//! shadcn's fixed roles (`theme().background`, `.card`, `.primary`, ...)
//! are aliases over the web contract (`card` = `var(--surface)`, `muted`
//! = `var(--surface-hover)`, `muted-foreground` = the web's `--muted`
//! text color, ...). The contract's own names that shadcn lacks are read
//! through exactly one function here each, so a rename is one edit.
//! Raw color values live only in `themes/` and in this file.

use nsh_base::theme;
use nsh_core::prelude::*;

/// The base theme file: the sacred palette in full.
pub const BASE_THEME: &str = include_str!("../../themes/theme.json");

/// A design-system palette (§4): `data-palette` on the web.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Palette {
    /// Cobalt accent; the web default (`DEFAULT_PALETTE`).
    #[default]
    Sacred,
    /// The ink itself is the accent (neutral).
    Ink,
    /// Magenta chrome over a neutral reader.
    Sepia,
    /// Emerald accent.
    Sapphire,
}

impl Palette {
    /// Every palette, in the web's picker order (`PALETTES`).
    pub const ALL: [Palette; 4] = [
        Palette::Sacred,
        Palette::Ink,
        Palette::Sepia,
        Palette::Sapphire,
    ];

    /// The id the web persists (`prefs.palette`).
    pub fn id(self) -> &'static str {
        match self {
            Palette::Sacred => "sacred",
            Palette::Ink => "ink",
            Palette::Sepia => "sepia",
            Palette::Sapphire => "sapphire",
        }
    }

    /// The palette of a persisted id; `None` for anything else.
    pub fn from_id(id: &str) -> Option<Palette> {
        Palette::ALL.into_iter().find(|p| p.id() == id)
    }

    /// The legacy `surface` id the web keeps in sync (`PALETTE_TO_SURFACE`).
    pub fn legacy_surface(self) -> &'static str {
        match self {
            Palette::Sacred => "mocha",
            Palette::Ink => "ink",
            Palette::Sepia => "paper",
            Palette::Sapphire => "slate",
        }
    }

    /// The palette a legacy `surface` id migrates to (`SURFACE_TO_PALETTE`).
    pub fn from_legacy_surface(surface: &str) -> Option<Palette> {
        match surface {
            "ink" | "contrast" => Some(Palette::Ink),
            "paper" => Some(Palette::Sepia),
            "slate" => Some(Palette::Sapphire),
            "mocha" => Some(Palette::Sacred),
            _ => None,
        }
    }

    /// The palette's override file (its accent family, light and dark).
    fn file(self) -> &'static str {
        match self {
            Palette::Sacred => include_str!("../../themes/palettes/sacred.json"),
            Palette::Ink => include_str!("../../themes/palettes/ink.json"),
            Palette::Sepia => include_str!("../../themes/palettes/sepia.json"),
            Palette::Sapphire => include_str!("../../themes/palettes/sapphire.json"),
        }
    }

    /// The accent swatch the appearance picker draws (`accentHex`): the
    /// palette's accent, which is what tells palettes apart.
    pub fn swatch(self) -> AdaptiveColor {
        let file: serde_json::Value = serde_json::from_str(self.file()).unwrap_or_default();
        let read = |mode: &str| {
            file["swatch"][mode]
                .as_str()
                .and_then(derive::parse_hex)
                .map(derive::Rgb::to_hsla)
                .unwrap_or_else(|| theme().primary.light)
        };
        AdaptiveColor::new(read("light"), read("dark"))
    }
}

/// The user's custom colors (`prefs.custom`): `#rrggbb` seeds the
/// derived tokens come from; each `None` keeps the palette's.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Seeds {
    /// The ground (background, surfaces, text, borders).
    pub bg: Option<String>,
    /// The interactive accent (the primary family, the focus ring).
    pub accent: Option<String>,
    /// The secondary hue (the web's editorial `--accent`, `gold` here).
    pub pop: Option<String>,
}

impl Seeds {
    /// Whether any seed is set (`prefs.hasCustom`).
    pub fn any(&self) -> bool {
        self.bg.is_some() || self.accent.is_some() || self.pop.is_some()
    }
}

/// The theme file of `palette` with `seeds` derived over it: the base
/// file, the palette's accent family in both modes, then every derived
/// token in both modes (the web sets them inline on `<html>`, over either
/// mode). Names the derived tokens write are the web's custom properties
/// without `--`; the legacy ramp (`--bg`, `--line`, ...) has no reader
/// here and is left out.
pub fn theme_json(palette: Palette, seeds: &Seeds) -> String {
    let mut base: serde_json::Value = serde_json::from_str(BASE_THEME).unwrap_or_default();
    let overrides: serde_json::Value = serde_json::from_str(palette.file()).unwrap_or_default();
    let derived = derive::derive_tokens(seeds);
    for mode in ["light", "dark"] {
        let Some(vars) = base["cssVars"][mode].as_object_mut() else {
            continue;
        };
        if let Some(palette_vars) = overrides[mode].as_object() {
            for (name, value) in palette_vars {
                vars.insert(name.clone(), value.clone());
            }
        }
        for (name, value) in &derived {
            vars.insert((*name).to_owned(), serde_json::Value::String(value.clone()));
        }
    }
    base.to_string()
}

/// The design tokens of `palette` with `seeds`: what
/// `nsh_core::element::set_tokens` takes.
pub fn tokens_for(palette: Palette, seeds: &Seeds) -> Result<nsh_core::element::Tokens, String> {
    nsh_base::tokens_of(Some(&theme_json(palette, seeds)))
}

/// `color` as the current appearance shows it: for a value that must be
/// one color (a caret, a placeholder), not for styles, which take the
/// adaptive pair and switch natively.
pub fn resolved(color: AdaptiveColor) -> Hsla {
    if dark() { color.dark } else { color.light }
}

/// The token's color, or a fallback that reads as a bug in review: a
/// misspelled token must not paint nothing.
fn token(name: &'static str, fallback: AdaptiveColor) -> AdaptiveColor {
    theme().color(name).unwrap_or(fallback)
}

/// A length token in points (`radius-pill`), or `fallback`.
fn length(name: &'static str, fallback: f32) -> Length {
    px(theme().number(name).unwrap_or(fallback)).into()
}

// --- Surfaces and text (§4) ------------------------------------------------

/// The quiet sub-surface: footer and band fills, inset fields.
pub fn background_subtle() -> AdaptiveColor {
    token("background-subtle", theme().background)
}

/// The content surface (cards, panels): also `theme().card`.
pub fn surface() -> AdaptiveColor {
    token("surface", theme().card)
}

/// The raised surface (popovers in light mode).
pub fn surface_raised() -> AdaptiveColor {
    token("surface-raised", theme().popover)
}

/// The hover and quiet fill: also `theme().muted`, `.secondary`, `.accent`.
pub fn surface_hover() -> AdaptiveColor {
    token("surface-hover", theme().muted)
}

/// Body text one step down from the foreground.
pub fn foreground_secondary() -> AdaptiveColor {
    token("foreground-secondary", theme().foreground)
}

/// The web's `--muted`: secondary text at the contrast floor (also
/// `theme().muted_foreground`).
pub fn muted_text() -> AdaptiveColor {
    token("muted-text", theme().muted_foreground)
}

/// The stronger hairline: inputs, chip edges, popup frames.
pub fn border_strong() -> AdaptiveColor {
    token("border-strong", theme().input)
}

// --- The accent family -------------------------------------------------------

/// The primary fill under the pointer.
pub fn primary_hover() -> AdaptiveColor {
    token("primary-hover", theme().primary)
}

/// The soft primary tint: selected pills, the revealed-ayah flash.
pub fn primary_soft() -> AdaptiveColor {
    token("primary-soft", theme().muted)
}

/// The keyboard focus ring (also `theme().ring`).
pub fn focus_ring() -> AdaptiveColor {
    token("focus-ring", theme().ring)
}

/// The secondary hue (the web's `text-gold`/`bg-gold`: the editorial
/// accent, hue 2 unless the user seeded it).
pub fn gold() -> AdaptiveColor {
    token("gold", theme().primary)
}

/// The strong secondary hue (Makki badge text).
pub fn gold_strong() -> AdaptiveColor {
    token("gold-strong", theme().primary)
}

/// The soft secondary hue (Makki badge fill).
pub fn gold_soft() -> AdaptiveColor {
    token("gold-soft", theme().muted)
}

// --- Status --------------------------------------------------------------

/// Text and icons drawn on `theme().destructive`.
pub fn destructive_foreground() -> AdaptiveColor {
    token("destructive-foreground", theme().background)
}

/// Success states: saved, online, synced.
pub fn success() -> AdaptiveColor {
    token("success", theme().foreground)
}

/// Text on `success()`.
pub fn success_foreground() -> AdaptiveColor {
    token("success-foreground", theme().background)
}

/// Warning states: stale data, storage almost full.
pub fn warning() -> AdaptiveColor {
    token("warning", theme().foreground)
}

/// Text on `warning()`.
pub fn warning_foreground() -> AdaptiveColor {
    token("warning-foreground", theme().background)
}

/// Danger and errors (also `theme().destructive`).
pub fn danger() -> AdaptiveColor {
    token("danger", theme().destructive)
}

/// Informational surfaces that are not errors (hue 1).
pub fn info() -> AdaptiveColor {
    token("info", theme().foreground)
}

/// Text on `info()`.
pub fn info_foreground() -> AdaptiveColor {
    token("info-foreground", theme().background)
}

/// A rising figure; data meaning, not decoration.
pub fn up() -> AdaptiveColor {
    token("up", theme().foreground)
}

/// A falling figure; data meaning, not decoration.
pub fn down() -> AdaptiveColor {
    token("down", theme().foreground)
}

/// The scrim behind a modal, a sheet or a popup (the web's `bg-black/20`).
pub fn backdrop() -> AdaptiveColor {
    token("backdrop", theme().foreground)
}

/// The tooltip chip: the web inverts it (`bg-foreground text-background`).
pub fn tooltip() -> AdaptiveColor {
    token("tooltip", theme().foreground)
}

/// Text on `tooltip()`.
pub fn tooltip_foreground() -> AdaptiveColor {
    token("tooltip-foreground", theme().background)
}

// --- The reader ------------------------------------------------------------

/// The reading column's ground.
pub fn reader_background() -> AdaptiveColor {
    token("reader-background", theme().background)
}

/// Quran text: the highest-contrast ink on the page.
pub fn quran_foreground() -> AdaptiveColor {
    token("quran-foreground", theme().foreground)
}

/// Translation text under an ayah.
pub fn translation_foreground() -> AdaptiveColor {
    token("translation-foreground", theme().foreground)
}

/// The hairline between ayahs.
pub fn reader_divider() -> AdaptiveColor {
    token("reader-divider", theme().border)
}

// --- The hue set (plan 01): card fills, the eight-color cycle ---------------

/// One of the eight brand hues, 1-8 (wraps), as a card fill.
pub fn hue(slot: u8) -> AdaptiveColor {
    token(HUES[hue_index(slot)], theme().primary)
}

/// Hue `slot`'s soft tint.
pub fn hue_soft(slot: u8) -> AdaptiveColor {
    token(HUE_SOFTS[hue_index(slot)], theme().muted)
}

/// Text and icons on hue `slot`'s fill.
pub fn on_hue(slot: u8) -> AdaptiveColor {
    token(ON_HUES[hue_index(slot)], theme().primary_foreground)
}

/// Hue `slot` legible as text on its soft chip (lighter in dark mode).
pub fn hue_legible(slot: u8) -> AdaptiveColor {
    token(HUE_LEGIBLES[hue_index(slot)], theme().foreground)
}

fn hue_index(slot: u8) -> usize {
    usize::from(slot.saturating_sub(1) % 8)
}

const HUES: [&str; 8] = [
    "hue-1", "hue-2", "hue-3", "hue-4", "hue-5", "hue-6", "hue-7", "hue-8",
];
const HUE_SOFTS: [&str; 8] = [
    "hue-1-soft",
    "hue-2-soft",
    "hue-3-soft",
    "hue-4-soft",
    "hue-5-soft",
    "hue-6-soft",
    "hue-7-soft",
    "hue-8-soft",
];
const ON_HUES: [&str; 8] = [
    "on-hue-1", "on-hue-2", "on-hue-3", "on-hue-4", "on-hue-5", "on-hue-6", "on-hue-7", "on-hue-8",
];
const HUE_LEGIBLES: [&str; 8] = [
    "hue-1-legible",
    "hue-2-legible",
    "hue-3-legible",
    "hue-4-legible",
    "hue-5-legible",
    "hue-6-legible",
    "hue-7-legible",
    "hue-8-legible",
];

// --- Tajweed (lib/quran/view/tajweed.ts RULE_COLORS) ------------------------

/// A tajweed rule class: the Dar Al-Islam edition's markup letter. The
/// Quran data area parses the markup; this is only how each class paints.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TajweedClass {
    /// `h` hamzat al-wasl.
    HamzatWasl,
    /// `s` silent letter.
    Silent,
    /// `l` lam shamsiyyah.
    LamShamsiyyah,
    /// `n` madda normal (2 counts).
    MaddaNormal,
    /// `p` madda permissible (2/4/6).
    MaddaPermissible,
    /// `m` madda necessary (6).
    MaddaNecessary,
    /// `o` madda obligatory (4-5).
    MaddaObligatory,
    /// `q` qalqalah.
    Qalqalah,
    /// `c` ikhfa shafawi.
    IkhfaShafawi,
    /// `f` ikhfa.
    Ikhfa,
    /// `w` idgham shafawi.
    IdghamShafawi,
    /// `i` iqlab.
    Iqlab,
    /// `a` idgham with ghunnah.
    IdghamGhunnah,
    /// `u` idgham without ghunnah.
    IdghamNoGhunnah,
    /// `d` idgham mutajanisayn / mutamathilayn.
    IdghamMutajanisayn,
    /// `b` idgham mutaqaribayn.
    IdghamMutaqaribayn,
    /// `g` ghunnah.
    Ghunnah,
}

impl TajweedClass {
    /// The class of a markup letter; `None` for a letter the web does not
    /// know (its run renders in the ambient color, never dropped).
    pub fn from_letter(letter: char) -> Option<TajweedClass> {
        Some(match letter {
            'h' => TajweedClass::HamzatWasl,
            's' => TajweedClass::Silent,
            'l' => TajweedClass::LamShamsiyyah,
            'n' => TajweedClass::MaddaNormal,
            'p' => TajweedClass::MaddaPermissible,
            'm' => TajweedClass::MaddaNecessary,
            'o' => TajweedClass::MaddaObligatory,
            'q' => TajweedClass::Qalqalah,
            'c' => TajweedClass::IkhfaShafawi,
            'f' => TajweedClass::Ikhfa,
            'w' => TajweedClass::IdghamShafawi,
            'i' => TajweedClass::Iqlab,
            'a' => TajweedClass::IdghamGhunnah,
            'u' => TajweedClass::IdghamNoGhunnah,
            'd' => TajweedClass::IdghamMutajanisayn,
            'b' => TajweedClass::IdghamMutaqaribayn,
            'g' => TajweedClass::Ghunnah,
            _ => return None,
        })
    }

    /// The markup letter.
    pub fn letter(self) -> char {
        match self {
            TajweedClass::HamzatWasl => 'h',
            TajweedClass::Silent => 's',
            TajweedClass::LamShamsiyyah => 'l',
            TajweedClass::MaddaNormal => 'n',
            TajweedClass::MaddaPermissible => 'p',
            TajweedClass::MaddaNecessary => 'm',
            TajweedClass::MaddaObligatory => 'o',
            TajweedClass::Qalqalah => 'q',
            TajweedClass::IkhfaShafawi => 'c',
            TajweedClass::Ikhfa => 'f',
            TajweedClass::IdghamShafawi => 'w',
            TajweedClass::Iqlab => 'i',
            TajweedClass::IdghamGhunnah => 'a',
            TajweedClass::IdghamNoGhunnah => 'u',
            TajweedClass::IdghamMutajanisayn => 'd',
            TajweedClass::IdghamMutaqaribayn => 'b',
            TajweedClass::Ghunnah => 'g',
        }
    }

    fn token(self) -> &'static str {
        match self {
            TajweedClass::HamzatWasl => "tajweed-h",
            TajweedClass::Silent => "tajweed-s",
            TajweedClass::LamShamsiyyah => "tajweed-l",
            TajweedClass::MaddaNormal => "tajweed-n",
            TajweedClass::MaddaPermissible => "tajweed-p",
            TajweedClass::MaddaNecessary => "tajweed-m",
            TajweedClass::MaddaObligatory => "tajweed-o",
            TajweedClass::Qalqalah => "tajweed-q",
            TajweedClass::IkhfaShafawi => "tajweed-c",
            TajweedClass::Ikhfa => "tajweed-f",
            TajweedClass::IdghamShafawi => "tajweed-w",
            TajweedClass::Iqlab => "tajweed-i",
            TajweedClass::IdghamGhunnah => "tajweed-a",
            TajweedClass::IdghamNoGhunnah => "tajweed-u",
            TajweedClass::IdghamMutajanisayn => "tajweed-d",
            TajweedClass::IdghamMutaqaribayn => "tajweed-b",
            TajweedClass::Ghunnah => "tajweed-g",
        }
    }
}

/// The color a tajweed class paints (the web's fixed convention, the same
/// in both modes).
pub fn tajweed(class: TajweedClass) -> AdaptiveColor {
    token(class.token(), quran_foreground())
}

// --- Geometry (§13 radius) ---------------------------------------------------

/// The icon holder: the square behind an icon (8).
pub fn radius_sm() -> Length {
    length("radius-sm", 8.0)
}

/// Blocks: rows, cards, popovers, sheets (10; also `theme().radius`).
pub fn radius_md() -> Length {
    length("radius-md", 10.0)
}

/// Large panels only (14).
pub fn radius_xl() -> Length {
    length("radius-xl", 14.0)
}

/// Controls: buttons, inputs, chips, tabs are pills.
pub fn radius_pill() -> Length {
    length("radius-pill", 999.0)
}

/// The headline highlight (16).
pub fn radius_highlight() -> Length {
    length("radius-highlight", 16.0)
}

/// `lib/theme/derive.ts`, ported as is: the tokens a custom background,
/// accent and pop seed derive. Pure (no engine); values are CSS strings
/// (`#rrggbb`, `rgba(r, g, b, a)`) exactly as the web writes them, which
/// [`theme_json`] places over the palette.
pub mod derive {
    use nsh_core::prelude::Hsla;

    use super::Seeds;

    /// An sRGB color, channels 0-255 (unclamped while mixing).
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Rgb {
        pub r: f64,
        pub g: f64,
        pub b: f64,
    }

    impl Rgb {
        /// As the engine's color.
        pub fn to_hsla(self) -> Hsla {
            nsh_core::color::Rgba {
                r: (channel(self.r) as f32) / 255.0,
                g: (channel(self.g) as f32) / 255.0,
                b: (channel(self.b) as f32) / 255.0,
                a: 1.0,
            }
            .to_hsla()
        }
    }

    const WHITE: Rgb = Rgb {
        r: 255.0,
        g: 255.0,
        b: 255.0,
    };
    const BLACK: Rgb = Rgb {
        r: 0.0,
        g: 0.0,
        b: 0.0,
    };

    /// `#rgb` or `#rrggbb` (the `#` optional), case-insensitive.
    pub fn parse_hex(hex: &str) -> Option<Rgb> {
        let digits = hex.trim();
        let digits = digits.strip_prefix('#').unwrap_or(digits);
        if !digits.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let full: String = match digits.len() {
            3 => digits.chars().flat_map(|c| [c, c]).collect(),
            6 => digits.to_owned(),
            _ => return None,
        };
        let at = |i: usize| u8::from_str_radix(&full[i..i + 2], 16).ok().map(f64::from);
        Some(Rgb {
            r: at(0)?,
            g: at(2)?,
            b: at(4)?,
        })
    }

    /// Whether `value` is a seed the web accepts (`/^#[0-9a-f]{6}$/i`).
    pub fn is_seed(value: &str) -> bool {
        value.len() == 7
            && value.starts_with('#')
            && value[1..].chars().all(|c| c.is_ascii_hexdigit())
    }

    /// `Math.round` then clamp to 0-255 (`toChannel`).
    fn channel(n: f64) -> u8 {
        // JS rounds half up; f64::round rounds half away from zero, which
        // only differs below zero, where the clamp makes both 0.
        n.round().clamp(0.0, 255.0) as u8
    }

    /// `#rrggbb`.
    pub fn to_hex(c: Rgb) -> String {
        format!(
            "#{:02x}{:02x}{:02x}",
            channel(c.r),
            channel(c.g),
            channel(c.b)
        )
    }

    /// WCAG relative luminance.
    pub fn luminance(c: Rgb) -> f64 {
        let ch = |v: f64| {
            let s = v / 255.0;
            if s <= 0.03928 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * ch(c.r) + 0.7152 * ch(c.g) + 0.0722 * ch(c.b)
    }

    /// The web's light/dark split of a seed.
    pub fn is_light(c: Rgb) -> bool {
        luminance(c) > 0.45
    }

    /// WCAG contrast ratio, (lighter + 0.05) / (darker + 0.05).
    pub fn contrast_ratio(a: Rgb, b: Rgb) -> f64 {
        let (la, lb) = (luminance(a), luminance(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    fn mix(a: Rgb, b: Rgb, t: f64) -> Rgb {
        Rgb {
            r: a.r + (b.r - a.r) * t,
            g: a.g + (b.g - a.g) * t,
            b: a.b + (b.b - a.b) * t,
        }
    }

    /// Toward white (`t > 0`) or black (`t < 0`).
    fn shift(c: Rgb, t: f64) -> Rgb {
        if t >= 0.0 {
            mix(c, WHITE, t)
        } else {
            mix(c, BLACK, -t)
        }
    }

    /// `rgba(r, g, b, a)` with channels rounded and clamped.
    fn rgba(c: Rgb, a: f64) -> String {
        format!(
            "rgba({}, {}, {}, {a})",
            channel(c.r),
            channel(c.g),
            channel(c.b)
        )
    }

    fn linear_to_channel(y: f64) -> f64 {
        if y <= 0.0031308 {
            y * 12.92
        } else {
            1.055 * y.powf(1.0 / 2.4) - 0.055
        }
    }

    /// The grey with the same WCAG luminance (plan 01: the ground is
    /// zero-chroma).
    fn to_neutral(seed: Rgb) -> Rgb {
        let c = f64::from(channel(linear_to_channel(luminance(seed)) * 255.0));
        Rgb { r: c, g: c, b: c }
    }

    /// A background seed: the contract's ground half.
    fn background_tokens(raw: Rgb, out: &mut Vec<(&'static str, String)>) {
        let seed = to_neutral(raw);
        let light = is_light(seed);
        let dir = if light { -1.0 } else { 1.0 };
        let ink = if light { BLACK } else { WHITE };
        let subtle = shift(seed, dir * 0.03);
        let surface = shift(seed, if light { 0.05 } else { 0.055 });
        let raised = shift(seed, if light { 0.1 } else { 0.09 });
        let hover = shift(seed, dir * if light { 0.055 } else { 0.1 });
        out.extend([
            ("background", to_hex(seed)),
            ("background-subtle", to_hex(subtle)),
            ("surface", to_hex(surface)),
            ("surface-raised", to_hex(raised)),
            ("surface-hover", to_hex(hover)),
            (
                "foreground",
                to_hex(mix(seed, ink, if light { 0.92 } else { 0.97 })),
            ),
            (
                "foreground-secondary",
                to_hex(mix(seed, ink, if light { 0.76 } else { 0.8 })),
            ),
            (
                "muted-text",
                to_hex(mix(seed, ink, if light { 0.6 } else { 0.64 })),
            ),
            ("border", rgba(ink, if light { 0.12 } else { 0.09 })),
            ("border-strong", rgba(ink, if light { 0.18 } else { 0.14 })),
            (
                "reader-background",
                to_hex(if light { raised } else { shift(seed, -0.2) }),
            ),
        ]);
    }

    /// An accent seed: the primary family; its on-color by luminance,
    /// never assumed white.
    fn accent_tokens(seed: Rgb, out: &mut Vec<(&'static str, String)>) {
        let on_light = is_light(seed);
        let on = if on_light {
            to_hex(mix(seed, BLACK, 0.88))
        } else {
            "#ffffff".to_owned()
        };
        out.extend([
            ("primary", to_hex(seed)),
            (
                "primary-hover",
                to_hex(shift(seed, if on_light { -0.08 } else { 0.08 })),
            ),
            ("primary-foreground", on.clone()),
            (
                "primary-soft",
                rgba(seed, if on_light { 0.12 } else { 0.16 }),
            ),
            ("focus-ring", to_hex(seed)),
            ("on-hue-1", on),
            // The legacy `--accent` ramp the accent seed also writes.
            ("gold", to_hex(seed)),
            ("gold-soft", rgba(seed, 0.13)),
        ]);
    }

    /// A pop seed: the secondary hue (written after the accent's, so it
    /// keeps ownership of `gold` when both are set).
    fn pop_tokens(seed: Rgb, out: &mut Vec<(&'static str, String)>) {
        out.extend([
            ("gold", to_hex(seed)),
            ("gold-strong", to_hex(seed)),
            ("gold-soft", rgba(seed, 0.13)),
        ]);
    }

    /// Every derived token of `seeds`, later entries winning (`deriveTokens`).
    pub fn derive_tokens(seeds: &Seeds) -> Vec<(&'static str, String)> {
        let mut out = Vec::new();
        if let Some(bg) = seeds.bg.as_deref().and_then(parse_hex) {
            background_tokens(bg, &mut out);
        }
        if let Some(accent) = seeds.accent.as_deref().and_then(parse_hex) {
            accent_tokens(accent, &mut out);
        }
        if let Some(pop) = seeds.pop.as_deref().and_then(parse_hex) {
            pop_tokens(pop, &mut out);
        }
        out
    }

    /// The derived tokens as the CSS the web's "Copy CSS" gives
    /// (`tokensToCss`), with the web's `--` names.
    pub fn tokens_to_css(tokens: &[(&'static str, String)], selector: &str) -> String {
        let body: Vec<String> = tokens
            .iter()
            .map(|(name, value)| format!("  --{}: {value};", web_name(name)))
            .collect();
        format!("{selector} {{\n{}\n}}", body.join("\n"))
    }

    /// The web's custom property for a token this app renamed.
    fn web_name(name: &str) -> &str {
        match name {
            "muted-text" => "muted",
            "gold" => "accent",
            "gold-strong" => "accent-strong",
            "gold-soft" => "accent-soft",
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every accessor's token exists in the base theme file: a rename on
    /// one side fails here instead of painting the fallback at run time.
    #[test]
    fn every_custom_token_is_declared() {
        let theme: serde_json::Value = serde_json::from_str(BASE_THEME).unwrap();
        for mode in ["light", "dark"] {
            let vars = theme["cssVars"][mode].as_object().unwrap();
            let mut names = vec![
                "background-subtle",
                "surface",
                "surface-raised",
                "surface-hover",
                "foreground-secondary",
                "muted-text",
                "border-strong",
                "primary-hover",
                "primary-soft",
                "focus-ring",
                "gold",
                "gold-strong",
                "gold-soft",
                "destructive-foreground",
                "success",
                "success-foreground",
                "warning",
                "warning-foreground",
                "danger",
                "info",
                "info-foreground",
                "up",
                "down",
                "backdrop",
                "tooltip",
                "tooltip-foreground",
                "reader-background",
                "quran-foreground",
                "translation-foreground",
                "reader-divider",
            ];
            names.extend(HUES);
            names.extend(HUE_SOFTS);
            names.extend(ON_HUES);
            names.extend(HUE_LEGIBLES);
            for letter in "hslnpmoqcfwiaudbg".chars() {
                names.push(TajweedClass::from_letter(letter).unwrap().token());
            }
            for name in names {
                assert!(vars.contains_key(name), "`{name}` missing from {mode}");
            }
        }
    }
}
