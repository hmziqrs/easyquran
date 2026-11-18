//! The appearance preferences (PARITY J2): light, dark or the system's
//! mode, the palette, and the custom seeds the derived tokens come from.
//! The web's `lib/stores/prefs.svelte.ts`, with the same persisted shape
//! under the same key (`easyquran.prefs`), so a synced or migrated
//! settings document reads the same on both.
//!
//! One owner: a surface-wide signal (`global`), so a settings page, the
//! menu bar and a shortcut write the same value from anywhere, listeners
//! and tasks included. [`AppearanceStore::install`] (once, from the
//! shell's wiring) loads the saved prefs and keeps the engine's tokens and
//! color scheme in step: palette and seeds through
//! `nsh_core::element::set_tokens`, the mode through `set_color_scheme`.

use nsh_core::element::set_tokens;
use nsh_core::platform::Appearance;
use nsh_core::prelude::*;
use nsh_core::storage::Storage;

use crate::ui::tokens::{self, Palette, Seeds};

/// The key the web keeps its prefs under (`STORAGE_KEY`).
pub const STORAGE_KEY: &str = "easyquran.prefs";

/// The appearance setting (§25): the tri-state the user picks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AppearanceMode {
    /// Always light.
    Light,
    /// Always dark.
    Dark,
    /// Follow the system (the web default, `DEFAULT_MODE`).
    #[default]
    System,
}

impl AppearanceMode {
    /// Every mode, in the web's order (`APPEARANCE_MODES`).
    pub const ALL: [AppearanceMode; 3] = [
        AppearanceMode::Light,
        AppearanceMode::Dark,
        AppearanceMode::System,
    ];

    /// The persisted id.
    pub fn id(self) -> &'static str {
        match self {
            AppearanceMode::Light => "light",
            AppearanceMode::Dark => "dark",
            AppearanceMode::System => "system",
        }
    }

    /// The mode of a persisted id.
    pub fn from_id(id: &str) -> Option<AppearanceMode> {
        AppearanceMode::ALL.into_iter().find(|m| m.id() == id)
    }

    fn engine(self) -> Appearance {
        match self {
            AppearanceMode::Light => Appearance::Light,
            AppearanceMode::Dark => Appearance::Dark,
            AppearanceMode::System => Appearance::System,
        }
    }
}

/// Which custom seed (`keyof CustomSeeds`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeedKey {
    /// The ground.
    Bg,
    /// The interactive accent.
    Accent,
    /// The secondary hue.
    Pop,
}

impl SeedKey {
    /// Every seed, in the web's order.
    pub const ALL: [SeedKey; 3] = [SeedKey::Bg, SeedKey::Accent, SeedKey::Pop];

    /// The persisted field name.
    pub fn id(self) -> &'static str {
        match self {
            SeedKey::Bg => "bg",
            SeedKey::Accent => "accent",
            SeedKey::Pop => "pop",
        }
    }
}

/// The persisted preferences (the web's `Prefs`).
#[derive(Clone, Debug, PartialEq)]
pub struct Prefs {
    /// The appearance setting.
    pub mode: AppearanceMode,
    /// The palette.
    pub palette: Palette,
    /// The custom seeds.
    pub custom: Seeds,
    /// The legacy accent id, only round-tripped (`emerald` by default).
    pub legacy_accent: String,
    /// Reopen the last reading position at launch (persisted here as on
    /// the web; the reading area owns what it does).
    pub instant_resume: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            mode: AppearanceMode::System,
            palette: Palette::Sacred,
            custom: Seeds::default(),
            legacy_accent: "emerald".to_owned(),
            instant_resume: false,
        }
    }
}

/// The legacy accent ids the web accepts (`ACCENTS`).
const LEGACY_ACCENTS: [&str; 4] = ["emerald", "gold", "azure", "plum"];

impl Prefs {
    /// The prefs of a stored JSON document, migrating as the web's `load`
    /// does: explicit new fields win, then legacy `theme` / `surface`.
    pub fn from_json(json: &str) -> Prefs {
        let base = Prefs::default();
        let Ok(stored) = serde_json::from_str::<serde_json::Value>(json) else {
            return base;
        };
        if !stored.is_object() {
            return base;
        }
        let text = |key: &str| stored[key].as_str();
        let mode = text("mode")
            .and_then(AppearanceMode::from_id)
            .or_else(|| {
                text("theme")
                    .filter(|t| *t == "light" || *t == "dark")
                    .and_then(AppearanceMode::from_id)
            })
            .unwrap_or(base.mode);
        let palette = text("palette")
            .and_then(Palette::from_id)
            .or_else(|| text("surface").and_then(Palette::from_legacy_surface))
            .unwrap_or(base.palette);
        let legacy_accent = text("accent")
            .filter(|a| LEGACY_ACCENTS.contains(a))
            .map_or(base.legacy_accent, str::to_owned);
        let seed = |key: &str| {
            stored["custom"][key]
                .as_str()
                .filter(|v| tokens::derive::is_seed(v))
                .map(str::to_owned)
        };
        Prefs {
            mode,
            palette,
            custom: Seeds {
                bg: seed("bg"),
                accent: seed("accent"),
                pop: seed("pop"),
            },
            legacy_accent,
            instant_resume: stored["instantResume"] == serde_json::Value::Bool(true),
        }
    }

    /// The stored JSON document, with the legacy fields the web keeps in
    /// sync (`theme` = the resolved mode, `surface` from the palette).
    pub fn to_json(&self, resolved_dark: bool) -> String {
        let mut custom = serde_json::Map::new();
        for key in SeedKey::ALL {
            if let Some(value) = self.seed(key) {
                custom.insert(key.id().to_owned(), value.into());
            }
        }
        serde_json::json!({
            "theme": if resolved_dark { "dark" } else { "light" },
            "surface": self.palette.legacy_surface(),
            "accent": self.legacy_accent,
            "palette": self.palette.id(),
            "mode": self.mode.id(),
            "custom": custom,
            "instantResume": self.instant_resume,
        })
        .to_string()
    }

    /// One seed.
    pub fn seed(&self, key: SeedKey) -> Option<&str> {
        match key {
            SeedKey::Bg => self.custom.bg.as_deref(),
            SeedKey::Accent => self.custom.accent.as_deref(),
            SeedKey::Pop => self.custom.pop.as_deref(),
        }
    }

    fn seed_mut(&mut self, key: SeedKey) -> &mut Option<String> {
        match key {
            SeedKey::Bg => &mut self.custom.bg,
            SeedKey::Accent => &mut self.custom.accent,
            SeedKey::Pop => &mut self.custom.pop,
        }
    }
}

/// The appearance store: a `Copy` handle over the surface's prefs signal.
/// Reads are tracked; every write persists.
#[derive(Clone, Copy)]
pub struct AppearanceStore {
    prefs: Signal<Prefs>,
}

impl AppearanceStore {
    /// The surface's store (any thread of the UI: bodies, listeners, tasks).
    pub fn get() -> AppearanceStore {
        AppearanceStore {
            prefs: global::<Prefs>(),
        }
    }

    /// Loads the saved prefs and keeps the engine's tokens and color
    /// scheme in step with them for as long as the caller's owner lives.
    /// Once, from the shell's wiring.
    pub fn install() -> AppearanceStore {
        let store = AppearanceStore::get();
        if let Some(saved) = Storage::get_item(STORAGE_KEY) {
            store.prefs.set(Prefs::from_json(&saved));
        }
        // Palette and seeds change the tokens; the mode alone does not.
        let look = computed(move || store.prefs.with(|p| (p.palette, p.custom.clone())));
        effect(move || {
            let (palette, seeds) = look.get();
            match tokens::tokens_for(palette, &seeds) {
                Ok(tokens) => set_tokens(tokens),
                Err(error) => log::warn!("appearance: theme not applied: {error}"),
            }
        });
        let mode = computed(move || store.prefs.with(|p| p.mode));
        effect(move || set_color_scheme(mode.get().engine()));
        store
    }

    /// The whole prefs value. Tracked.
    pub fn prefs(&self) -> Prefs {
        self.prefs.get()
    }

    /// The appearance setting. Tracked.
    pub fn mode(&self) -> AppearanceMode {
        self.prefs.with(|p| p.mode)
    }

    /// The palette. Tracked.
    pub fn palette(&self) -> Palette {
        self.prefs.with(|p| p.palette)
    }

    /// The custom seeds. Tracked.
    pub fn custom(&self) -> Seeds {
        self.prefs.with(|p| p.custom.clone())
    }

    /// Whether any custom seed is set. Tracked.
    pub fn has_custom(&self) -> bool {
        self.prefs.with(|p| p.custom.any())
    }

    /// Whether the surface shows dark now (the mode resolved against the
    /// system). Tracked.
    pub fn is_dark(&self) -> bool {
        dark()
    }

    /// The instant-resume preference. Tracked.
    pub fn instant_resume(&self) -> bool {
        self.prefs.with(|p| p.instant_resume)
    }

    /// The derived tokens as CSS, as the web's "Copy CSS" gives them.
    pub fn css(&self) -> String {
        let prefs = self.prefs.get_untracked();
        let scheme = if dark() { "dark" } else { "light" };
        let selector = format!(
            "[data-palette=\"{}\"][data-mode=\"{scheme}\"]",
            prefs.palette.id()
        );
        tokens::derive::tokens_to_css(&tokens::derive::derive_tokens(&prefs.custom), &selector)
    }

    /// Sets the appearance mode.
    pub fn set_mode(&self, mode: AppearanceMode) {
        self.write(|p| p.mode = mode);
    }

    /// Flips between light and dark (from whatever shows now).
    pub fn toggle_mode(&self) {
        let next = if dark() {
            AppearanceMode::Light
        } else {
            AppearanceMode::Dark
        };
        self.set_mode(next);
    }

    /// Sets the palette.
    pub fn set_palette(&self, palette: Palette) {
        self.write(|p| p.palette = palette);
    }

    /// Sets the legacy accent id (it also drops a custom accent seed, as
    /// the web's `setAccent` does).
    pub fn set_legacy_accent(&self, accent: &str) {
        if !LEGACY_ACCENTS.contains(&accent) {
            return;
        }
        self.write(|p| {
            p.legacy_accent = accent.to_owned();
            p.custom.accent = None;
        });
    }

    /// Sets one seed (`#rrggbb`), or clears it with `None` or a value that
    /// is not one.
    pub fn set_custom(&self, key: SeedKey, hex: Option<&str>) {
        let value = hex
            .filter(|v| tokens::derive::is_seed(v))
            .map(str::to_owned);
        self.write(|p| *p.seed_mut(key) = value);
    }

    /// Clears every seed.
    pub fn clear_custom(&self) {
        self.write(|p| p.custom = Seeds::default());
    }

    /// Sets the instant-resume preference.
    pub fn set_instant_resume(&self, value: bool) {
        self.write(|p| p.instant_resume = value);
    }

    /// Back to the defaults.
    pub fn reset(&self) {
        self.write(|p| *p = Prefs::default());
    }

    fn write(&self, change: impl FnOnce(&mut Prefs)) {
        let mut next = self.prefs.get_untracked();
        change(&mut next);
        let resolved_dark = match next.mode {
            AppearanceMode::Light => false,
            AppearanceMode::Dark => true,
            AppearanceMode::System => dark(),
        };
        if let Err(error) = Storage::set_item(STORAGE_KEY, next.to_json(resolved_dark)) {
            log::warn!("appearance: not saved: {error:?}");
        }
        self.prefs.set(next);
    }
}
