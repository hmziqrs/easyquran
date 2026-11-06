import { readFileSync } from "node:fs";

import { describe, expect, it } from "vite-plus/test";

import { getAccountCopy, type AccountCopy } from "#lib/i18n/account-copy.js";

const ARABIC_LETTERS = /\p{Script=Arabic}/u;

// Non-text fields: locale metadata and the one parameterized formatter.
const NON_TEXT_KEYS = new Set(["locale", "direction", "sessionLastSeen"]);

function textFields(copy: AccountCopy): Map<string, string> {
  const fields = new Map<string, string>();
  for (const [key, value] of Object.entries(copy)) {
    if (NON_TEXT_KEYS.has(key)) continue;
    // SAFETY: getAccountCopy returns only string fields outside NON_TEXT_KEYS, so this value is a string.
    fields.set(key, value as string);
  }
  return fields;
}

describe("account copy", () => {
  const en = getAccountCopy("en");
  const ar = getAccountCopy("ar");
  const enText = textFields(en);

  it("gives every en string a non-empty value", () => {
    for (const [key, value] of enText) {
      expect(value.trim(), `en.${key}`).not.toBe("");
    }
  });

  it("localizes every ar string into Arabic that differs from en", () => {
    for (const [key, value] of textFields(ar)) {
      expect(value.trim(), `ar.${key}`).not.toBe("");
      expect(ARABIC_LETTERS.test(value), `ar.${key} must be Arabic: ${value}`).toBe(true);
      expect(value, `ar.${key} must differ from en`).not.toBe(enText.get(key));
    }
  });

  it("keeps the interpolation parameter in both locales", () => {
    expect(en.sessionLastSeen("X")).toContain("X");
    expect(ar.sessionLastSeen("X")).toContain("X");
  });

  it("sets lang-direction from the locale", () => {
    expect(en.locale).toBe("en");
    expect(en.direction).toBe("ltr");
    expect(ar.locale).toBe("ar");
    expect(ar.direction).toBe("rtl");
  });

  it("keeps catalogs key-identical across en and ar", () => {
    const keys = (path: string): string[] =>
      Object.keys(
        // SAFETY: the catalogs are repo-owned flat JSON objects of string values.
        JSON.parse(readFileSync(new URL(path, import.meta.url), "utf8")) as Record<string, string>,
      ).sort();
    expect(keys("../../../../messages/account/ar.json")).toEqual(
      keys("../../../../messages/account/en.json"),
    );
  });
});
