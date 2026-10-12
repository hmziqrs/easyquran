import { browser } from "$app/env";
import type { ConsentSettings } from "firebase/analytics";

import { asObject, readJSON, writeJSON } from "#lib/storage/index.js";

const STORAGE_KEY = "easyquran.consent";
/** Exported for cross-tab opt-out listeners (reading analytics drops its queues). */
export { STORAGE_KEY as CONSENT_STORAGE_KEY };

export interface ConsentFlags {
  analytics: boolean;
  performance: boolean;
  advertising: boolean;
  /**
   * First-party reading analytics (Feature 6): which surah/ayah you read, kept
   * on our server when signed in. OFF by default forever — a per-ayah reading
   * history is religious practice (GDPR Art. 9), so it needs an explicit opt-in
   * separate from the GA `analytics` flag and is never bulk-granted.
   */
  readingAnalytics: boolean;
}

type ConsentPatch = Partial<ConsentFlags>;

const DEFAULT_FLAGS: ConsentFlags = {
  analytics: true,
  performance: true,
  advertising: false,
  readingAnalytics: false,
};

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the untyped localStorage JSON boundary (readJSON → JSON.parse); this function is the parser (asObject + strict field comparisons)
export function decodeConsent(raw: unknown): ConsentFlags {
  const stored = asObject(raw);
  if (!stored) return { ...DEFAULT_FLAGS };
  return {
    analytics: stored.analytics === false ? false : DEFAULT_FLAGS.analytics,
    performance: stored.performance === false ? false : DEFAULT_FLAGS.performance,
    advertising: stored.advertising === true ? true : DEFAULT_FLAGS.advertising,
    readingAnalytics: stored.readingAnalytics === true,
  };
}

export interface Consent {
  readonly current: Readonly<ConsentFlags>;
  readonly analytics: boolean;
  readonly performance: boolean;
  readonly advertising: boolean;
  readonly readingAnalytics: boolean;
  readonly consentSettings: ConsentSettings;
  set(patch: ConsentPatch): void;
  setAnalytics(on: boolean): void;
  setPerformance(on: boolean): void;
  setReadingAnalytics(on: boolean): void;
  allowAll(): void;
  denyAll(): void;
  hydrate(): void;
}

export function createConsent(): Consent {
  let flags = $state<ConsentFlags>({ ...DEFAULT_FLAGS });
  let hydrated = false;

  const set = (patch: ConsentPatch): void => {
    if (patch.analytics !== undefined) flags.analytics = patch.analytics;
    if (patch.performance !== undefined) flags.performance = patch.performance;
    if (patch.advertising !== undefined) flags.advertising = patch.advertising;
    if (patch.readingAnalytics !== undefined) flags.readingAnalytics = patch.readingAnalytics;
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
    get readingAnalytics(): boolean {
      return flags.readingAnalytics;
    },
    get consentSettings(): ConsentSettings {
      const g = (on: boolean) => (on ? "granted" : "denied");
      return {
        ad_storage: g(flags.advertising),
        ad_user_data: g(flags.advertising),
        ad_personalization: g(flags.advertising),
        analytics_storage: g(flags.analytics),
        functionality_storage: "granted",
        security_storage: "granted",
      };
    },
    set,
    setAnalytics: (on: boolean) => set({ analytics: on }),
    setPerformance: (on: boolean) => set({ performance: on }),
    setReadingAnalytics: (on: boolean) => set({ readingAnalytics: on }),
    // Bulk actions never grant the special-category reading flag; deny-all
    // revokes everything (privacy direction only).
    allowAll: () => set({ analytics: true, performance: true, advertising: true }),
    denyAll: () =>
      set({ analytics: false, performance: false, advertising: false, readingAnalytics: false }),
    hydrate(): void {
      if (hydrated || !browser) return;
      hydrated = true;
      flags = { ...decodeConsent(readJSON(STORAGE_KEY)) };
    },
  };
}

export const consent = createConsent();
