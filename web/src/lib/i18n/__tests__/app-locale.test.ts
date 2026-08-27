import { readFileSync } from "node:fs";

import { mount, unmount } from "svelte";
import { describe, expect, it } from "vite-plus/test";

import AppLocaleHarness from "./app-locale-harness.svelte";

function source(path: string): string {
  return readFileSync(new URL(path, import.meta.url), "utf8");
}

function harnessText(target: HTMLElement): string {
  return target.querySelector("p")?.textContent ?? "";
}

function harnessAttributes(target: HTMLElement) {
  const p = target.querySelector("p");
  return { locale: p?.dataset.locale ?? "", direction: p?.dataset.direction ?? "" };
}

describe("app-locale hand-off", () => {
  it("follows the published layout locale on the de-localized bookmarks page copy", () => {
    const target = document.createElement("div");
    document.body.append(target);
    const harness = mount(AppLocaleHarness, { target, props: { locale: "ar" } });
    try {
      const attributes = harnessAttributes(target);
      expect(attributes.locale).toBe("ar");
      expect(attributes.direction).toBe("rtl");
      expect(harnessText(target)).toContain("العلامات المرجعية");
      expect(harnessText(target)).toContain("غير مصنّفة");
      expect(harnessText(target)).toContain("المجلدات");
    } finally {
      void unmount(harness);
      target.remove();
    }
  });

  it("falls back to the base English locale when no layout published one", () => {
    const target = document.createElement("div");
    document.body.append(target);
    const harness = mount(AppLocaleHarness, { target });
    try {
      const attributes = harnessAttributes(target);
      expect(attributes.locale).toBe("en");
      expect(attributes.direction).toBe("ltr");
      expect(harnessText(target)).toContain("Bookmarks");
      expect(harnessText(target)).toContain("Unfiled");
      expect(harnessText(target)).toContain("Folders");
    } finally {
      void unmount(harness);
      target.remove();
    }
  });

  it("wires the layout publisher and the bookmarks page consumer together", () => {
    const layout = source("../../../routes/(application)/app/+layout.svelte");
    expect(layout).toContain("setAppLocale(copy.locale)");

    const page = source("../../../routes/(application)/app/bookmarks/+page.svelte");
    expect(page).toContain("getBookmarksCopy(appLocale())");

    // Settings is English-by-design: it must not start consuming the hand-off.
    const settings = source("../../../routes/(application)/app/settings/+page.svelte");
    expect(settings).not.toContain("appLocale");
  });
});
