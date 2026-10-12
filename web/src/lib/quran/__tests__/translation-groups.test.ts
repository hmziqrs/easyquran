import { describe, expect, it } from "vite-plus/test";

import { TRANSLATIONS } from "#lib/data/translations.js";
import {
  groupTranslationAuthors,
  normalizedTranslatorKey,
  translationAuthorKey,
  translationProvenanceRank,
  type TranslationAuthorEntry,
} from "#lib/quran/translation-groups.js";

const entry = (
  id: string,
  languageCode: string,
  translator: string | null,
): TranslationAuthorEntry => ({ id, languageCode, translator });

describe("author grouping derivation", () => {
  it("normalizes translator identity: trim, casefold, collapse whitespace", () => {
    expect(normalizedTranslatorKey("  Muhammad   Hamidullah ")).toBe("muhammad hamidullah");
    expect(normalizedTranslatorKey("MUHAMMAD HAMIDULLAH")).toBe("muhammad hamidullah");
    expect(normalizedTranslatorKey(null)).toBe("");
  });

  it("groups same-language variants of one author, keeping given order", () => {
    const groups = groupTranslationAuthors([
      entry("en.pickthall", "en", "Marmaduke Pickthall"),
      entry("qul.r19.quran-en-pickthall", "en", "marmaduke   pickthall"),
      entry("en.wahiduddin", "en", "Wahiduddin Khan"),
    ]);
    expect(groups.map((g) => g.variants.map((v) => v.id))).toEqual([
      ["en.pickthall", "qul.r19.quran-en-pickthall"],
      ["en.wahiduddin"],
    ]);
  });

  it("never merges unnamed translators or different languages", () => {
    expect(translationAuthorKey(entry("a.one", "en", null))).not.toBe(
      translationAuthorKey(entry("a.two", "en", null)),
    );
    expect(translationAuthorKey(entry("fr.x", "fr", "Hamidullah"))).not.toBe(
      translationAuthorKey(entry("en.x", "en", "Hamidullah")),
    );
  });

  it("keeps an attribution mismatch separate (name vs translator credit)", () => {
    // quranenc.en.saheeh is credited to Noor International Center although its
    // display name is Saheeh International: different translator, own group.
    const groups = groupTranslationAuthors([
      entry("en.sahih", "en", "Saheeh International"),
      entry("qul.r20.en-sahih-international", "en", "Saheeh International"),
      entry("quranenc.en.saheeh", "en", "Noor International Center"),
    ]);
    expect(groups).toHaveLength(2);
    expect(groups[0]?.variants).toHaveLength(2);
    expect(groups[1]?.variants.map((v) => v.id)).toEqual(["quranenc.en.saheeh"]);
  });

  it("prefers original publishers over QUL mirrors for the default variant", () => {
    expect(translationProvenanceRank("en.sahih")).toBeLessThan(
      translationProvenanceRank("qul.r20.en-sahih-international"),
    );
    expect(translationProvenanceRank("quranenc.ja.sato")).toBeLessThan(
      translationProvenanceRank("qul.r218.ja-saeed"),
    );
  });
});

// ux-audit guardrail: exact duplicate works (same language, name, and author)
// across sources are allowed only for this frozen allow-list, keyed by the
// same normalization the picker groups with. A new catalogue duplicate fails
// here; removing one of the frozen rows fails too.
const FROZEN_EXACT_DUPLICATE_PAIRS: readonly (readonly string[])[] = [
  ["en.sahih", "qul.r20.en-sahih-international"],
  ["quranenc.ja.sato", "qul.r218.ja-saeed"],
  ["quranenc.pt.nasr", "qul.r103.pt-helmi"],
  ["quranenc.sv.rwwad", "qul.r1627.sv-rwwad"],
  ["quranenc.ta.omar_brief", "qul.r1633.ta-omar-brief"],
  ["quranenc.uz.rwwad", "qul.r868.uz-rowwad"],
  ["ro.grigore", "qul.r44.ro-grigore"],
];

describe("baked catalogue — duplicate-work guard", () => {
  const byId = new Map(TRANSLATIONS.map((t) => [t.id, t]));
  const duplicatePairs = (() => {
    const exactGroups = new Map<string, string[]>();
    for (const t of TRANSLATIONS) {
      const key = JSON.stringify([t.language, t.name, normalizedTranslatorKey(t.translator)]);
      const ids = exactGroups.get(key) ?? [];
      ids.push(t.id);
      exactGroups.set(key, ids);
    }
    return [...exactGroups.values()]
      .filter((ids) => ids.length > 1)
      .map((ids) => JSON.stringify([...ids].sort()))
      .sort();
  })();

  it("holds exactly the frozen seven exact-duplicate pairs, no more", () => {
    expect(duplicatePairs).toEqual(
      FROZEN_EXACT_DUPLICATE_PAIRS.map((pair) => JSON.stringify([...pair].sort())).sort(),
    );
  });

  it("every frozen pair still groups under one author key", () => {
    for (const pair of FROZEN_EXACT_DUPLICATE_PAIRS) {
      const first = byId.get(pair[0] ?? "");
      const second = byId.get(pair[1] ?? "");
      expect(first).toBeDefined();
      expect(second).toBeDefined();
      if (first !== undefined && second !== undefined) {
        expect(translationAuthorKey(first)).toBe(translationAuthorKey(second));
      }
    }
  });

  it("the attribution mismatch stays its own author (never merged into Saheeh)", () => {
    const saheeh = byId.get("quranenc.en.saheeh");
    const sahih = byId.get("en.sahih");
    if (saheeh !== undefined && sahih !== undefined) {
      expect(translationAuthorKey(saheeh)).not.toBe(translationAuthorKey(sahih));
    }
  });
});
