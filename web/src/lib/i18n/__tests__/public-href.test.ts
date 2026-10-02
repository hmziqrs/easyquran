import { publicHref } from "$lib/i18n/public-href";
import { describe, expect, it } from "vite-plus/test";

describe("public localized href resolution", () => {
  it("keeps bounded origin-relative localized paths intact", () => {
    expect(publicHref("/ar/")).toBe("/ar/");
    expect(publicHref("/al-fatihah?view=focus#ayah-1-1")).toBe(
      "/al-fatihah?view=focus#ayah-1-1",
    );
    expect(publicHref("/ar/al-fatihah?view=focus#ayah-1-1")).toBe(
      "/ar/al-fatihah?view=focus#ayah-1-1",
    );
  });

  it.each(["//evil.test/x", "/al-fatihah\\evil", "/al-fatihah\nnext"] as const)(
    "rejects unsafe public href: %s",
    (href) => {
      expect(() => publicHref(href)).toThrow(TypeError);
    },
  );
});
