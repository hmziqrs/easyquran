import { describe, expect, it } from "vite-plus/test";
import { publicHref } from "$lib/i18n/public-href";

describe("public localized href resolution", () => {
  it("keeps bounded origin-relative localized paths intact", () => {
    expect(publicHref("/ar/")).toBe("/ar/");
    expect(publicHref("/en/app/al-fatihah?view=focus#ayah-1-1")).toBe(
      "/en/app/al-fatihah?view=focus#ayah-1-1",
    );
  });

  it.each(["//evil.test/app", "/en/app\\evil", "/en/app\nnext"] as const)(
    "rejects unsafe public href: %s",
    (href) => {
      expect(() => publicHref(href)).toThrow(TypeError);
    },
  );
});
