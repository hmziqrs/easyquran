import { browser } from "$app/env";
import type { ConsentSettings } from "firebase/analytics";

import { asObject, readJSON, writeJSON } from "#lib/storage/index.js";

const STORAGE_KEY = "easyquran.consent";
export { STORAGE_KEY as CONSENT_STORAGE_KEY };

export interface ConsentFlags {
  analytics: boolean;
  advertising: boolean;
}

type ConsentPatch = Partial<ConsentFlags>;

/**
 * Off until the visitor turns it on in Settings. Personal reading insights are
 * product data, not consent data, and are not gated here.
 */
const DEFAULT_FLAGS: ConsentFlags = {
  analytics: false,
  advertising: false,
};

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the untyped localStorage JSON boundary (readJSON → JSON.parse); this function is the parser (asObject + strict field comparisons)
export function decodeConsent(raw: unknown): ConsentFlags {
  const stored = asObject(raw);
  if (!stored) return { ...DEFAULT_FLAGS };
  return {
    analytics: stored.analytics === true ? true : DEFAULT_FLAGS.analytics,
    advertising: stored.advertising === true ? true : DEFAULT_FLAGS.advertising,
  };
}

export interface Consent {
  readonly current: Readonly<ConsentFlags>;
  /** Stored preference for a non-minor account. Drives the Settings toggle. */
  readonly analytics: boolean;
  readonly advertising: boolean;
  /** Analytics may leave the device now: preference on, not a minor, auth status resolved. */
  readonly analyticsAllowed: boolean;
  readonly consentSettings: ConsentSettings;
  set(patch: ConsentPatch): void;
  setAnalytics(on: boolean): void;
  allowAll(): void;
  denyAll(): void;
  hydrate(): void;
  /** Signed-in account is a declared minor (A34): analytics and ads stay off, whatever is stored. */
  setMinorAccount(on: boolean): void;
  /**
   * Auth status has left "unknown" (A34). Until then analytics and ads are held off, so a
   * signed-in minor's first page view cannot leave before the flag is known. Latches once set.
   */
  markAuthResolved(): void;
}

export function createConsent(): Consent {
  let flags = $state<ConsentFlags>({ ...DEFAULT_FLAGS });
  let hydrated = false;
  let minorAccount = $state(false);
  let authResolved = $state(false);

  const set = (patch: ConsentPatch): void => {
    if (patch.analytics !== undefined) flags.analytics = patch.analytics;
    if (patch.advertising !== undefined) flags.advertising = patch.advertising;
    if (browser) {
      writeJSON(STORAGE_KEY, flags);
      window.dispatchEvent(new CustomEvent("easyquran:consent", { detail: patch }));
    }
  };

  /** Analytics and ads are held while the account is a minor or its status is still unknown. */
  const held = (): boolean => minorAccount || !authResolved;

  return {
    get current(): Readonly<ConsentFlags> {
      return flags;
    },
    get analytics(): boolean {
      return flags.analytics && !minorAccount;
    },
    get advertising(): boolean {
      return flags.advertising && !minorAccount;
    },
    get analyticsAllowed(): boolean {
      return flags.analytics && !held();
    },
    get consentSettings(): ConsentSettings {
      const g = (on: boolean) => (on ? "granted" : "denied");
      return {
        ad_storage: g(flags.advertising && !held()),
        ad_user_data: g(flags.advertising && !held()),
        ad_personalization: g(flags.advertising && !held()),
        analytics_storage: g(flags.analytics && !held()),
        functionality_storage: "granted",
        security_storage: "granted",
      };
    },
    set,
    setAnalytics: (on: boolean) => set({ analytics: on }),
    allowAll: () => set({ analytics: true, advertising: true }),
    denyAll: () => set({ analytics: false, advertising: false }),
    hydrate(): void {
      if (hydrated || !browser) return;
      hydrated = true;
      flags = { ...decodeConsent(readJSON(STORAGE_KEY)) };
    },
    setMinorAccount(on: boolean): void {
      if (minorAccount === on) return;
      minorAccount = on;
      if (browser) window.dispatchEvent(new CustomEvent("easyquran:consent", { detail: {} }));
    },
    markAuthResolved(): void {
      if (authResolved) return;
      authResolved = true;
      if (browser) window.dispatchEvent(new CustomEvent("easyquran:consent", { detail: {} }));
    },
  };
}

export const consent = createConsent();
