/* ════════════════════════════════════════════════════════════════════════
   consent.svelte.ts — the user's data-consent choices.

   A single Svelte 5 runes class, SSR-safe (every DOM/localStorage access is
   guarded behind `browser`). Persists to localStorage and broadcasts a custom
   event on change:
     • "easyquran:consent" — a consent flag changed (the root layout applies it
       to Firebase: GA4 consent mode + collection toggles).

   This store is deliberately Firebase-agnostic — it holds booleans and emits.
   The wiring (consent → analytics/performance) lives in the layout, which keeps
   the store free of import cycles.

   Defaults reflect the project's stance (analytics/performance enabled, plainly
   disclosed in the Privacy Policy); users can opt out at any time in Settings.
   ════════════════════════════════════════════════════════════════════════ */

import { browser } from "$app/environment";
import type { ConsentSettings } from "firebase/analytics";

const STORAGE_KEY = "easyquran.consent";

export interface ConsentFlags {
  /** Firebase Analytics (GA4) — page views, feature usage, events. */
  analytics: boolean;
  /** Firebase Performance Monitoring — load/network traces. */
  performance: boolean;
  /** Advertising-related storage. We run no ads today; kept for forward-compat. */
  advertising: boolean;
}

type ConsentPatch = Partial<ConsentFlags>;

const DEFAULT_FLAGS: ConsentFlags = { analytics: true, performance: true, advertising: false };

function load(): ConsentFlags {
  if (!browser) return { ...DEFAULT_FLAGS };
  try {
    const stored = JSON.parse(localStorage.getItem(STORAGE_KEY) || "{}");
    return { ...DEFAULT_FLAGS, ...stored } as ConsentFlags;
  } catch {
    return { ...DEFAULT_FLAGS };
  }
}

class ConsentStore {
  // SSR renders from DEFAULTS; saved flags hydrate after mount.
  #flags = $state<ConsentFlags>({ ...DEFAULT_FLAGS });
  #hydrated = false;

  /** Hydrate from localStorage after mount (SSR used DEFAULTS). */
  hydrate(): void {
    if (this.#hydrated || !browser) return;
    this.#hydrated = true;
    this.#flags = { ...this.#flags, ...load() };
  }

  get current(): Readonly<ConsentFlags> {
    return this.#flags;
  }
  get analytics(): boolean {
    return this.#flags.analytics;
  }
  get performance(): boolean {
    return this.#flags.performance;
  }
  get advertising(): boolean {
    return this.#flags.advertising;
  }

  /** GA4 consent-mode settings derived from the current flags. */
  get consentSettings(): ConsentSettings {
    const g = (on: boolean) => (on ? "granted" : "denied");
    return {
      ad_storage: g(this.#flags.advertising),
      ad_user_data: g(this.#flags.advertising),
      ad_personalization: g(this.#flags.advertising),
      analytics_storage: g(this.#flags.analytics),
      // Always-on: needed for the site to function / stay secure.
      functionality_storage: "granted",
      security_storage: "granted",
    };
  }

  /** Patch one or more flags, persist, and notify listeners. */
  set(patch: ConsentPatch): void {
    this.#flags = { ...this.#flags, ...patch };
    if (browser) {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(this.#flags));
      window.dispatchEvent(new CustomEvent("easyquran:consent", { detail: patch }));
    }
  }

  setAnalytics(on: boolean): void {
    this.set({ analytics: on });
  }
  setPerformance(on: boolean): void {
    this.set({ performance: on });
  }

  allowAll(): void {
    this.set({ analytics: true, performance: true, advertising: true });
  }
  denyAll(): void {
    this.set({ analytics: false, performance: false, advertising: false });
  }
}

export const consent = new ConsentStore();
