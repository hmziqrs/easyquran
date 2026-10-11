import { beforeEach, describe, expect, it, vi } from "vitest";

const env = vi.hoisted(() => ({ publicEnv: "prod" }));

vi.mock("$app/env", () => ({ browser: true, dev: false }));
vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: undefined,
  PUBLIC_QURAN_API_BASE: undefined,
  PUBLIC_FCM_VAPID_KEY: undefined,
  get PUBLIC_ENV() {
    return env.publicEnv;
  },
}));

import { DEFAULTS } from "#lib/config/site.js";
import { QURAN_SOURCE_IDS, type QuranSourceId } from "#lib/data/quran-types.js";
import { QuranDataEnvironment } from "#lib/quran/environment.js";
import {
  enabledScriptIds,
  isScriptEnabled,
  PRODUCTION_DISABLED_SOURCES,
} from "#lib/quran/script-availability.js";
import {
  applySettingsDoc,
  SETTINGS_DOC_VERSION,
  type SettingsConsentPort,
  type SettingsDoc,
  type SettingsPrefsPort,
} from "#lib/settings/settings-document.js";
import { createReaderCore, READER_DEFAULTS } from "#lib/stores/reader-core.svelte.js";
import { createReaderPersistence } from "#lib/stores/reader-persistence.svelte.js";
import { createReaderSettings } from "#lib/stores/reader-settings.svelte.js";

const PROD = QuranDataEnvironment.Production;
const LOCAL = QuranDataEnvironment.Local;
const READER_KEY = "easyquran.reader";

function fakePrefs(): SettingsPrefsPort {
  return {
    current: { ...DEFAULTS, instantResume: false, custom: {} },
    set() {},
    apply() {},
  };
}

function fakeConsent(): SettingsConsentPort {
  return {
    current: { analytics: true, performance: true, advertising: false },
    set() {},
  };
}

function settingsDoc(arabicScript: QuranSourceId): SettingsDoc {
  return {
    v: SETTINGS_DOC_VERSION,
    appearance: { ...DEFAULTS, instantResume: false, custom: {} },
    reading: {
      fontSize: 33,
      mode: "verse",
      arabicFont: "kfgqpc-hafs",
      arabicScript,
      translationSize: 17,
    },
    privacy: { analytics: true, performance: true, advertising: false },
  };
}

describe("script availability", () => {
  it("gates indopak and tajweed, and only in production", () => {
    expect([...PRODUCTION_DISABLED_SOURCES].sort()).toEqual(["indopak", "tajweed"]);
    for (const id of QURAN_SOURCE_IDS) {
      expect(isScriptEnabled(id, LOCAL)).toBe(true);
    }
    expect(isScriptEnabled("uthmani", PROD)).toBe(true);
    expect(isScriptEnabled("uthmani-annotated", PROD)).toBe(true);
    expect(isScriptEnabled("simple-clean", PROD)).toBe(true);
    expect(isScriptEnabled("indopak", PROD)).toBe(false);
    expect(isScriptEnabled("tajweed", PROD)).toBe(false);
  });

  it("lists enabled scripts in source order", () => {
    expect(enabledScriptIds(LOCAL)).toEqual([
      "uthmani",
      "uthmani-annotated",
      "simple-clean",
      "indopak",
      "tajweed",
    ]);
    expect(enabledScriptIds(PROD)).toEqual(["uthmani", "uthmani-annotated", "simple-clean"]);
  });
});

describe("reader script guards", () => {
  beforeEach(() => {
    env.publicEnv = "prod";
    window.localStorage.clear();
  });

  it("refuses production-disabled scripts at the setter, allows them locally", () => {
    const core = createReaderCore();
    const settings = createReaderSettings(core, createReaderPersistence(core));
    settings.setArabicScript("indopak");
    settings.setArabicScript("tajweed");
    expect(core.s.arabicScript).toBe(READER_DEFAULTS.arabicScript);
    settings.setArabicScript("uthmani");
    expect(core.s.arabicScript).toBe("uthmani");

    env.publicEnv = "local";
    settings.setArabicScript("indopak");
    expect(core.s.arabicScript).toBe("indopak");
  });

  it("falls back on hydrate in production but keeps the stored preference", () => {
    window.localStorage.setItem(READER_KEY, JSON.stringify({ v: 4, arabicScript: "indopak" }));
    const core = createReaderCore();
    const persistence = createReaderPersistence(core);
    persistence.hydrate();
    expect(core.s.arabicScript).toBe(READER_DEFAULTS.arabicScript);
    expect(window.localStorage.getItem(READER_KEY)).toContain('"indopak"');
    persistence.dispose();
  });

  it("hydrates the stored script locally", () => {
    env.publicEnv = "local";
    window.localStorage.setItem(READER_KEY, JSON.stringify({ v: 4, arabicScript: "indopak" }));
    const core = createReaderCore();
    const persistence = createReaderPersistence(core);
    persistence.hydrate();
    expect(core.s.arabicScript).toBe("indopak");
    persistence.dispose();
  });

  it("filters a production-disabled script when applying a settings doc", () => {
    const core = createReaderCore();
    const stores = {
      prefs: fakePrefs(),
      consent: fakeConsent(),
      reader: createReaderSettings(core, createReaderPersistence(core)),
    };
    applySettingsDoc(settingsDoc("indopak"), stores);
    expect(core.s.arabicScript).toBe(READER_DEFAULTS.arabicScript);

    env.publicEnv = "local";
    applySettingsDoc(settingsDoc("indopak"), stores);
    expect(core.s.arabicScript).toBe("indopak");
  });
});
