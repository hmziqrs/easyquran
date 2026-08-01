/* ════════════════════════════════════════════════════════════════════════
   prefs.svelte.ts — user preferences (theme, surface, accent, custom colours).

   A single Svelte 5 runes class, SSR-safe (guards every DOM/localStorage
   access behind `browser`). Persists to localStorage, applies the choice to
   <html>, and broadcasts a custom event:
     • "easyquran:pref" — any pref changed (settings UI re-syncs)

   Three orthogonal dials drive the palette:
     • theme   — dark | light            → [data-theme]
     • surface — background family        → [data-surface]
     • accent  — brand colour family      → [data-accent]
   …plus an optional `custom` layer of up to three seed colours (background,
   accent, pop). Those are expanded into a full token set by lib/theme/derive
   and written as an INLINE STYLE on <html>, which outranks every selector in
   layout.css — so a custom accent overrides the accent family while leaving
   the chosen surface family untouched.
   ════════════════════════════════════════════════════════════════════════ */

import { browser } from "$app/environment";
import {
  ACCENTS,
  DEFAULTS,
  SURFACES,
  type AccentId,
  type SurfaceId,
  type ThemeMode,
} from "$lib/config/site";
import { deriveTokens, tokensToCss, type CustomSeeds } from "$lib/theme/derive";

const STORAGE_KEY = "easyquran.prefs";

/** Every CSS property the custom layer can ever write. Listed so `apply()` can
 *  clear a property that was just unset, instead of leaving it stuck. */
const CUSTOM_PROPS = [
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
  "--accent",
  "--accent-soft",
  "--accent-line",
  "--accent-fg",
  "--ring",
  "--pop",
  "--pop-soft",
] as const;

export interface Prefs {
  theme: ThemeMode;
  surface: SurfaceId;
  accent: AccentId;
  custom: CustomSeeds;
}

type PrefPatch = Partial<Prefs>;

const isHex = (v: unknown): v is string => typeof v === "string" && /^#[0-9a-f]{6}$/i.test(v);

function cleanCustom(raw: unknown): CustomSeeds {
  const out: CustomSeeds = {};
  if (!raw || typeof raw !== "object") return out;
  const c = raw as Record<string, unknown>;
  if (isHex(c.bg)) out.bg = c.bg;
  if (isHex(c.accent)) out.accent = c.accent;
  if (isHex(c.pop)) out.pop = c.pop;
  return out;
}

function load(): Prefs {
  const base: Prefs = { ...DEFAULTS, custom: {} };
  if (!browser) return base;
  try {
    const stored = JSON.parse(localStorage.getItem(STORAGE_KEY) || "{}");
    return {
      theme: stored?.theme === "dark" || stored?.theme === "light" ? stored.theme : base.theme,
      surface: SURFACES.some((s) => s.id === stored?.surface) ? stored.surface : base.surface,
      accent: ACCENTS.some((a) => a.id === stored?.accent) ? stored.accent : base.accent,
      custom: cleanCustom(stored?.custom),
    };
  } catch {
    return base;
  }
}

class PrefsStore {
  // SSR renders from DEFAULTS; saved prefs hydrate after mount so the theme
  // icon (and any pref-derived UI) matches the prerendered HTML on first paint.
  #prefs = $state<Prefs>({ ...DEFAULTS, custom: {} });
  #hydrated = false;

  hydrate(): void {
    if (this.#hydrated || !browser) return;
    this.#hydrated = true;
    this.#prefs = { ...this.#prefs, ...load() };
  }

  get current(): Readonly<Prefs> {
    return this.#prefs;
  }
  get theme(): ThemeMode {
    return this.#prefs.theme;
  }
  get surface(): SurfaceId {
    return this.#prefs.surface;
  }
  get accent(): AccentId {
    return this.#prefs.accent;
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
    const selector =
      `[data-theme="${this.#prefs.theme}"][data-surface="${this.#prefs.surface}"]` +
      `[data-accent="${this.#prefs.accent}"]`;
    return tokensToCss(this.customTokens, selector);
  }

  apply(): void {
    if (!browser) return;
    const el = document.documentElement;
    el.dataset.theme = this.#prefs.theme;
    el.dataset.surface = this.#prefs.surface;
    el.dataset.accent = this.#prefs.accent;

    // Custom layer: write what's set, clear what isn't, so unsetting a seed
    // falls back to the preset rules rather than sticking at its last value.
    const tokens = this.customTokens;
    for (const prop of CUSTOM_PROPS) {
      const value = tokens[prop];
      if (value) el.style.setProperty(prop, value);
      else el.style.removeProperty(prop);
    }
  }

  set(patch: PrefPatch): void {
    this.#prefs = { ...this.#prefs, ...patch };
    if (browser) {
      try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(this.#prefs));
      } catch {
        /* storage may be unavailable (private mode, quota) — non-fatal */
      }
      this.apply();
      window.dispatchEvent(new CustomEvent("easyquran:pref", { detail: patch }));
    }
  }

  setTheme(theme: ThemeMode): void {
    this.set({ theme });
  }
  setSurface(surface: SurfaceId): void {
    this.set({ surface });
  }
  setAccent(accent: AccentId): void {
    // Picking a preset accent clears a custom accent seed, otherwise the click
    // would appear to do nothing (the inline style would keep winning).
    const { accent: _dropped, ...rest } = this.#prefs.custom;
    this.set({ accent, custom: rest });
  }
  toggleTheme(): void {
    this.set({ theme: this.#prefs.theme === "dark" ? "light" : "dark" });
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
    this.set({ ...DEFAULTS, custom: {} });
  }
}

export const prefs = new PrefsStore();
