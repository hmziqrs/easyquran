/* ════════════════════════════════════════════════════════════════════════
   prefs.svelte.ts — user preferences (theme, accent).

   A single Svelte 5 runes class, SSR-safe (guards every DOM/localStorage
   access behind `browser`). Persists to localStorage, applies the choice to
   <html> data-attributes, and broadcasts a custom event:
     • "easyquran:pref" — any pref changed (settings UI re-syncs)
   ════════════════════════════════════════════════════════════════════════ */

import { browser } from "$app/environment";
import { DEFAULTS, type AccentId, type ThemeMode } from "$lib/config/site";

const STORAGE_KEY = "easyquran.prefs";

export interface Prefs {
  theme: ThemeMode;
  accent: AccentId;
}

type PrefPatch = Partial<Prefs>;

function load(): Prefs {
  if (!browser) return { ...DEFAULTS } as Prefs;
  try {
    const stored = JSON.parse(localStorage.getItem(STORAGE_KEY) || "{}");
    return { ...DEFAULTS, ...stored } as Prefs;
  } catch {
    return { ...DEFAULTS } as Prefs;
  }
}

class PrefsStore {
  // SSR renders from DEFAULTS; saved prefs hydrate after mount so the theme
  // icon (and any pref-derived UI) matches the prerendered HTML on first paint.
  #prefs = $state<Prefs>({ ...DEFAULTS } as Prefs);
  #hydrated = false;

  /** Hydrate from localStorage after mount (SSR used DEFAULTS). */
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
  get accent(): AccentId {
    return this.#prefs.accent;
  }

  /** Apply the current prefs to <html> data-attributes (idempotent). */
  apply(): void {
    if (!browser) return;
    const el = document.documentElement;
    el.dataset.theme = this.#prefs.theme;
    el.dataset.accent = this.#prefs.accent;
  }

  /** Patch one or more prefs, persist, apply, and notify listeners. */
  set(patch: PrefPatch): void {
    this.#prefs = { ...this.#prefs, ...patch };
    if (browser) {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(this.#prefs));
      this.apply();
      window.dispatchEvent(new CustomEvent("easyquran:pref", { detail: patch }));
    }
  }

  setTheme(theme: ThemeMode): void {
    this.set({ theme });
  }
  setAccent(accent: AccentId): void {
    this.set({ accent });
  }
  toggleTheme(): void {
    this.set({ theme: this.#prefs.theme === "dark" ? "light" : "dark" });
  }
}

export const prefs = new PrefsStore();
