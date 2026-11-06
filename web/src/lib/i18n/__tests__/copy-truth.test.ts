import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { describe, expect, it } from "vite-plus/test";

import { QURAN_SOURCE_IDS } from "#lib/data/quran-types.js";
import { QuranDataEnvironment } from "#lib/quran/environment.js";
import { enabledScriptIds } from "#lib/quran/script-availability.js";

/**
 * C1 copy truth pass: shipped copy must match what the code enforces. Each claim
 * below is pinned to the code it describes, so a feature change fails here first.
 */

/** Parses a message catalogue into a key -> value map. */
const readCatalog = (path: string): Map<string, unknown> => {
  const parsed: object = JSON.parse(readFileSync(resolve(process.cwd(), path), "utf8"));
  return new Map<string, unknown>(Object.entries(parsed));
};

const en = readCatalog("messages/en.json");
const ar = readCatalog("messages/ar.json");
const authEn = readCatalog("messages/auth/en.json");
const authAr = readCatalog("messages/auth/ar.json");

const text = (catalog: ReadonlyMap<string, unknown>, key: string): string => {
  const value = catalog.get(key);
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- test helper: guards the JSON catalogue boundary before reading a string message.
  if (typeof value !== "string") throw new Error(`missing string message ${key}`);
  return value;
};

describe("C1 copy truth pass", () => {
  it("drops the unused auth_merge_* strings from both locales", () => {
    for (const catalog of [authEn, authAr]) {
      expect([...catalog.keys()].filter((key) => key.startsWith("auth_merge_"))).toEqual([]);
    }
  });

  it("names exactly the scripts production serves", () => {
    const productionScripts = enabledScriptIds(QuranDataEnvironment.Production);
    expect(productionScripts.length).toBe(3);
    expect(productionScripts.length).toBeLessThan(QURAN_SOURCE_IDS.length);
    expect(text(en, "about_feature_scripts")).toContain("three scripts");
    expect(text(en, "about_feature_scripts")).not.toContain("IndoPak");
    expect(text(en, "about_feature_scripts")).not.toContain("Tajweed");
  });

  it("promises no recitation audio in the offline answer until F19 ships", () => {
    expect(text(en, "faq_offline_answer")).not.toMatch(/recitation|audio/i);
    expect(text(ar, "faq_offline_answer")).not.toContain("صوت");
  });

  it("claims no automatic translation removal, since no code path evicts downloads", () => {
    for (const catalog of [en, ar]) {
      expect(text(catalog, "settings_storage_cap_note")).not.toMatch(/automatic|تلقائي/u);
      expect(text(catalog, "settings_storage_retention_note")).not.toMatch(/30|٣٠|automatic/u);
    }
  });

  it("describes the sepia palette without a background it does not render", () => {
    expect(text(en, "settings_palette_sepia_note")).not.toMatch(/warm|reading page/i);
    expect(text(en, "tweaks_palette_sepia_note")).not.toMatch(/warm|reading page/i);
  });

  it("keeps the translation-source counts equal to the baked catalogue", () => {
    // SAFETY: translations.json is the baked catalogue; each row is [id, language, code, dir, ...].
    const catalogue = JSON.parse(
      readFileSync(resolve(process.cwd(), "src/lib/data/translations.json"), "utf8"),
    ) as string[][];
    const sources = catalogue.length;
    const languages = new Set(catalogue.map((row) => row[1])).size;
    expect(sources).toBe(378);
    expect(languages).toBe(105);
    expect(text(en, "faq_translations_answer")).toContain(`${sources} translation sources`);
    expect(text(en, "faq_translations_answer")).toContain(`${languages} languages`);
    const arabicTafsir = catalogue.filter((row) => /^ar\.(jalalayn|muyassar)$/.test(row[0] ?? ""));
    expect(arabicTafsir.length).toBe(2);
    expect(text(en, "faq_translations_answer")).toContain("Two Arabic tafsir sources");
  });
});
