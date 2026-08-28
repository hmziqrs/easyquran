import { browser } from "$app/environment";
import {
  ACCENTS,
  DEFAULT_MODE,
  DEFAULT_PALETTE,
  PALETTES,
  PALETTE_TO_SURFACE,
  SURFACES,
  SURFACE_TO_PALETTE,
  type AccentId,
  type AppearanceMode,
  type PaletteId,
  type SurfaceId,
  type ThemeMode,
} from "$lib/config/site";
import { asLiteral, asObject, asString, onStorageKey, readJSON, writeJSON } from "$lib/storage";
import { deriveTokens, tokensToCss, type CustomSeeds } from "$lib/theme/derive";

const STORAGE_KEY = "easyquran.prefs";

const CUSTOM_PROPS = [
  // §4 semantic contract (docs/design-system.md) — custom seeds override these directly…
  "--background",
  "--background-subtle",
  "--surface",
  "--surface-raised",
  "--surface-hover",
  "--foreground",
  "--foreground-secondary",
  "--muted",
  "--border",
  "--border-strong",
  "--reader-background",
  "--primary",
  "--primary-hover",
  "--primary-foreground",
  "--primary-soft",
  "--focus-ring",
  "--accent",
  "--accent-strong",
  "--accent-soft",
  // …and the legacy ramp below is kept so older sheets/aliases stay consistent until swept.
  "--bg",
  "--bg-1",
  "--bg-2",
  "--bg-3",
  "--bg-elev",
  "--line",
  "--line-2",
  "--line-3",
  "--fg",
  "--fg-2",
  "--fg-3",
  "--fg-4",
  "--accent-line",
  "--accent-fg",
  "--ring",
  "--pop",
  "--pop-soft",
] as const;

export interface Prefs {
  /**
   * Legacy persisted mode (dark|light). Kept because settings-document.ts (frozen this round)
   * decodes it; the store always keeps it equal to the *resolved* appearance mode.
   */
  theme: ThemeMode;
  /** Legacy surface id. Kept in sync with `palette` via PALETTE_TO_SURFACE for older sync paths. */
  surface: SurfaceId;
  /** Legacy accent id. Palettes own accent colors now; this field only round-trips old prefs. */
  accent: AccentId;
  /** Design-system palette (§4). Optional so the frozen settings-document decoder stays assignable. */
  palette?: PaletteId;
  /** Appearance setting (§25): light/dark/system. `theme` mirrors the resolved value. */
  mode?: AppearanceMode;
  custom: CustomSeeds;
  instantResume: boolean;
}

type PrefPatch = Partial<Prefs>;

// eslint-disable-next-line anti-slop/no-unknown-parameters -- guards asObject() dictionary values (raw localStorage JSON); the string check below is the parse
const isHex = (v: unknown): v is string => {
  const s = asString(v);
  return s !== undefined && /^#[0-9a-f]{6}$/i.test(s);
};

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the untyped localStorage JSON boundary (asObject value); this function is the parser (isHex validates each seed)
function cleanCustom(raw: unknown): CustomSeeds {
  const out: CustomSeeds = {};
  const c = asObject(raw);
  if (!c) return out;
  if (isHex(c.bg)) out.bg = c.bg;
  if (isHex(c.accent)) out.accent = c.accent;
  if (isHex(c.pop)) out.pop = c.pop;
  return out;
}

function systemPrefersDark(): boolean {
  if (!browser) return true;
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

/** Resolves the §25 appearance setting to the concrete light/dark value the CSS attributes need. */
export function resolveMode(mode: AppearanceMode, fallback: ThemeMode): ThemeMode {
  if (mode === "system") return systemPrefersDark() ? "dark" : "light";
  return mode === "light" || mode === "dark" ? mode : fallback;
}

function load(): Prefs {
  const base: Prefs = {
    theme: "dark",
    surface: PALETTE_TO_SURFACE[DEFAULT_PALETTE],
    accent: "emerald",
    palette: DEFAULT_PALETTE,
    mode: DEFAULT_MODE,
    instantResume: false,
    custom: {},
  };
  if (!browser) return base;
  const stored = asObject(readJSON(STORAGE_KEY));
  if (!stored) return base;

  // Back-compat (§25 migration): surface→palette, theme→mode; explicit new fields win.
  const legacySurface = SURFACES.find((s) => s.id === stored.surface)?.id;
  const mode: AppearanceMode =
    asLiteral(stored.mode, ["light", "dark", "system"] as const) ??
    asLiteral(stored.theme, ["light", "dark"] as const) ??
    DEFAULT_MODE;
  const palette: PaletteId =
    PALETTES.find((p) => p.id === stored.palette)?.id ??
    (legacySurface ? SURFACE_TO_PALETTE[legacySurface] : undefined) ??
    DEFAULT_PALETTE;
  return {
    theme: resolveMode(mode, base.theme),
    surface: PALETTE_TO_SURFACE[palette],
    accent: ACCENTS.find((a) => a.id === stored.accent)?.id ?? base.accent,
    palette,
    mode,
    custom: cleanCustom(stored.custom),
    instantResume: stored.instantResume === true,
  };
}

function loadDefaults(): Prefs {
  return {
    theme: "dark",
    surface: PALETTE_TO_SURFACE[DEFAULT_PALETTE],
    accent: "emerald",
    palette: DEFAULT_PALETTE,
    mode: DEFAULT_MODE,
    instantResume: false,
    custom: {},
  };
}

class PrefsStore {
  #prefs = $state<Prefs>(loadDefaults());
  #hydrated = false;

  hydrate(): void {
    if (this.#hydrated || !browser) return;
    this.#hydrated = true;
    this.#prefs = { ...this.#prefs, ...load() };
    onStorageKey(STORAGE_KEY, () => {
      this.#prefs = load();
      this.apply();
    });
    // §25 "System": follow the OS while no explicit light/dark choice is active.
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    mq.addEventListener("change", () => {
      if (this.#prefs.mode === "system") this.apply();
    });
  }

  get current(): Readonly<Prefs> {
    return this.#prefs;
  }
  /** The concrete active mode (never "system"). Nav and legacy consumers read this. */
  get theme(): ThemeMode {
    return resolveMode(this.mode, this.#prefs.theme);
  }
  get surface(): SurfaceId {
    return this.#prefs.surface;
  }
  get accent(): AccentId {
    return this.#prefs.accent;
  }
  get palette(): PaletteId {
    return this.#prefs.palette ?? DEFAULT_PALETTE;
  }
  get mode(): AppearanceMode {
    return this.#prefs.mode ?? DEFAULT_MODE;
  }
  get instantResume(): boolean {
    return this.#prefs.instantResume;
  }
  get custom(): Readonly<CustomSeeds> {
    return this.#prefs.custom;
  }
  get hasCustom(): boolean {
    const c = this.#prefs.custom;
    return Boolean(c.bg || c.accent || c.pop);
  }

  get customTokens(): Record<string, string> {
    return deriveTokens(this.#prefs.custom);
  }

  css(): string {
    const selector = `[data-palette="${this.palette}"][data-mode="${this.theme}"]`;
    return tokensToCss(this.customTokens, selector);
  }

  apply(): void {
    if (!browser) return;
    const el = document.documentElement;
    el.dataset.palette = this.palette;
    el.dataset.mode = this.theme;
    // Legacy attribute names are gone from layout.css; scrub them so stale markup from a cached
    // app.html can't resurrect the retired [data-theme]/[data-surface]/[data-accent] blocks.
    delete el.dataset.theme;
    delete el.dataset.surface;
    delete el.dataset.accent;

    const tokens = this.customTokens;
    for (const prop of CUSTOM_PROPS) {
      const value = tokens[prop];
      if (value) el.style.setProperty(prop, value);
      else el.style.removeProperty(prop);
    }
  }

  set(patch: PrefPatch): void {
    const next: Prefs = { ...this.#prefs, ...patch };
    if (next.palette) next.surface = PALETTE_TO_SURFACE[next.palette];
    if (patch.mode) next.theme = resolveMode(patch.mode, next.theme);
    this.#prefs = next;
    if (browser) {
      writeJSON(STORAGE_KEY, this.#prefs);
      this.apply();
      window.dispatchEvent(new CustomEvent("easyquran:pref", { detail: patch }));
    }
  }

  /** @deprecated legacy name — sets the appearance mode (light/dark). Prefer setMode. */
  setTheme(theme: ThemeMode): void {
    this.setMode(theme);
  }
  setMode(mode: AppearanceMode): void {
    this.set({ mode, theme: resolveMode(mode, this.theme) });
  }
  /** @deprecated legacy name — palettes replaced surfaces; kept for older callers. */
  setSurface(surface: SurfaceId): void {
    this.setPalette(SURFACE_TO_PALETTE[surface]);
  }
  setPalette(palette: PaletteId): void {
    this.set({ palette });
  }
  setAccent(accent: AccentId): void {
    const { accent: _dropped, ...rest } = this.#prefs.custom;
    this.set({ accent, custom: rest });
  }
  setInstantResume(value: boolean): void {
    this.set({ instantResume: value });
  }
  toggleTheme(): void {
    this.setMode(this.theme === "dark" ? "light" : "dark");
  }

  setCustom(key: keyof CustomSeeds, hex: string | undefined): void {
    const next = { ...this.#prefs.custom };
    if (hex && isHex(hex)) next[key] = hex;
    else delete next[key];
    this.set({ custom: next });
  }

  clearCustom(): void {
    this.set({ custom: {} });
  }

  reset(): void {
    this.set(loadDefaults());
  }
}

export const prefs = new PrefsStore();
