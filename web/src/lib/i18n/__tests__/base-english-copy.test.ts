import { MARKETING_PAGES } from "$lib/config/site-structure";
import { baseEnglishPageCopy, baseEnglishPageCopyForPath } from "$lib/i18n/base-english-copy";
import { describe, expect, it } from "vite-plus/test";

describe("base-English representation copy", () => {
  it("keeps structural routes separate from render-time English copy", () => {
    expect(MARKETING_PAGES.every((page) => !Object.hasOwn(page, "label"))).toBe(true);
    expect(baseEnglishPageCopy("about").label).toBe("About");
    expect(baseEnglishPageCopyForPath("/about")?.title).toBe("About · EasyQuran");
    expect(baseEnglishPageCopyForPath("/missing")).toBeNull();
  });
});
