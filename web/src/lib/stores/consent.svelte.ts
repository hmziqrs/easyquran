/* ════════════════════════════════════════════════════════════════════════
   consent.svelte.ts — the user's data-consent choices.

   A Svelte 5 runes store, SSR-safe (every DOM/localStorage access is guarded
   behind `browser`). Persists to localStorage and broadcasts a custom event on
   change:
     • "easyquran:consent" — a consent flag changed (the root layout applies it
       to Firebase: GA4 consent mode + collection toggles).

   This store is deliberately Firebase-agnostic — it holds booleans and emits.
   The wiring (consent → analytics/performance) lives in the layout, which keeps
   the store free of import cycles. Persistence mechanics come from
   $lib/storage; policy: consent persists IMMEDIATELY, then the Firebase consent
   bridge (in the layout) reacts to the broadcast.

   Defaults reflect the project's stance (analytics/performance enabled, plainly
   disclosed in the Privacy Policy); users can opt out at any time in Settings.
   ════════════════════════════════════════════════════════════════════════ */

import { browser } from "$app/environment";
import type { ConsentSettings } from "firebase/analytics";
import { asObject, readJSON, writeJSON } from "$lib/storage";

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

/** Validate a raw localStorage blob into full flags, preserving the original
 *  default-on/default-off semantics: analytics/performance default ON unless
 *  explicitly `false`; advertising defaults OFF unless explicitly `true`.
 *  Non-boolean values keep the default. */
export function decodeConsent(raw: unknown): ConsentFlags {
  const stored = asObject(raw);
  if (!stored) return { ...DEFAULT_FLAGS };
  return {
    analytics: stored.analytics === false ? false : DEFAULT_FLAGS.analytics,
    performance: stored.performance === false ? false : DEFAULT_FLAGS.performance,
    advertising: stored.advertising === true ? true : DEFAULT_FLAGS.advertising,
  };
}

export interface Consent {
  readonly current: Readonly<ConsentFlags>;
  readonly analytics: boolean;
  readonly performance: boolean;
  readonly advertising: boolean;
  /** GA4 consent-mode settings derived from the current flags. */
  readonly consentSettings: ConsentSettings;
  /** Patch one or more flags, persist, and notify listeners. */
  set(patch: ConsentPatch): void;
  setAnalytics(on: boolean): void;
  setPerformance(on: boolean): void;
  allowAll(): void;
  denyAll(): void;
  /** Hydrate from localStorage after mount (SSR used DEFAULT_FLAGS). */
  hydrate(): void;
}

export function createConsent(): Consent {
  // SSR renders from DEFAULT_FLAGS; saved flags hydrate after mount.
  let flags = $state<ConsentFlags>({ ...DEFAULT_FLAGS });
  let hydrated = false;

  const set = (patch: ConsentPatch): void => {
    if (patch.analytics !== undefined) flags.analytics = patch.analytics;
    if (patch.performance !== undefined) flags.performance = patch.performance;
    if (patch.advertising !== undefined) flags.advertising = patch.advertising;
    if (browser) {
      writeJSON(STORAGE_KEY, flags);
      window.dispatchEvent(new CustomEvent("easyquran:consent", { detail: patch }));
    }
  };

  return {
    get current(): Readonly<ConsentFlags> {
      return flags;
    },
    get analytics(): boolean {
      return flags.analytics;
    },
    get performance(): boolean {
      return flags.performance;
    },
    get advertising(): boolean {
      return flags.advertising;
    },
    get consentSettings(): ConsentSettings {
      const g = (on: boolean) => (on ? "granted" : "denied");
      return {
        ad_storage: g(flags.advertising),
        ad_user_data: g(flags.advertising),
        ad_personalization: g(flags.advertising),
        analytics_storage: g(flags.analytics),
        // Always-on: needed for the site to function / stay secure.
        functionality_storage: "granted",
        security_storage: "granted",
      };
    },
    set,
    setAnalytics: (on: boolean) => set({ analytics: on }),
    setPerformance: (on: boolean) => set({ performance: on }),
    allowAll: () => set({ analytics: true, performance: true, advertising: true }),
    denyAll: () => set({ analytics: false, performance: false, advertising: false }),
    hydrate(): void {
      if (hydrated || !browser) return;
      hydrated = true;
      flags = { ...decodeConsent(readJSON(STORAGE_KEY)) };
    },
  };
}

export const consent = createConsent();
