import { readFileSync } from "node:fs";
import { describe, expect, it } from "vite-plus/test";

function source(path: string): string {
  return readFileSync(new URL(path, import.meta.url), "utf8");
}

describe("marketing localization boundaries", () => {
  it("keeps marketing-copy free of messages so type-only importers stay copy-free", () => {
    const copySource = source("../marketing-copy.ts");

    expect(copySource).not.toContain("$lib/paraglide");
    expect(copySource).not.toContain("$lib/i18n/m/");
    expect(copySource).not.toMatch(/messages\/(?:en|ar)\.json/);
  });

  it("resolves each namespace from its own generated barrel", () => {
    const chrome = source("../chrome-copy.ts");
    const appearance = source("../appearance-copy.ts");
    const landing = source("../landing-copy.ts");

    expect(chrome).toContain('from "$lib/i18n/m/chrome"');
    expect(chrome).not.toContain('from "$lib/i18n/m/landing"');
    expect(chrome).not.toContain('from "$lib/i18n/m/appearance"');
    expect(appearance).toContain('from "$lib/i18n/m/appearance"');
    expect(appearance).not.toContain('from "$lib/i18n/m/chrome"');
    expect(landing).toContain('from "$lib/i18n/m/landing"');
    expect(landing).not.toContain('from "$lib/i18n/m/chrome"');
    for (const module of [chrome, appearance, landing]) {
      expect(module).toContain("{ locale }");
    }
  });

  it("loads both appearance panels lazily", () => {
    const marketingTweaks = source("../../../routes/(marketing)/_components/MarketingTweaks.svelte");
    const readerLayout = source("../../../routes/(application)/app/+layout.svelte");
    const tweaks = source("../../components/tweaks/Tweaks.svelte");

    expect(marketingTweaks).toContain('await import("$lib/i18n/appearance-copy")');
    expect(readerLayout).toContain('await import("$lib/i18n/reader-settings-copy")');
    expect(tweaks).toContain("loadCopy");
    expect(tweaks).not.toContain("DEFAULT_COPY");
  });

  it("keeps shared chrome independent from Paraglide runtime", () => {
    const shared = [
      source("../../components/nav/Nav.svelte"),
      source("../../components/footer/Footer.svelte"),
      source("../../components/tweaks/Tweaks.svelte"),
      source("../../components/brand/Brand.svelte"),
    ].join("\n");

    expect(shared).not.toContain("$lib/paraglide");
    expect(shared).not.toMatch(/messages\/(?:en|ar)\.json/);
  });

  it("keeps Quran quotation verbatim and explicitly Arabic RTL", () => {
    const footer = source("../../components/footer/Footer.svelte");

    expect(footer).toContain('lang="ar" dir="rtl"');
    expect(footer).toContain("وَنَزَّلْنَا عَلَيْكَ الْكِتَابَ تِبْيَانًا لِّكُلِّ شَيْءٍ");
  });

  it("routes landing reader links through the reader home helper", () => {
    const landing = source("../../../routes/(marketing)/+page.svelte");

    expect(landing).toContain("marketingReaderHomeHref(locale)");
    expect(landing).not.toMatch(/href=["']\/(?:en|ar)?\/?app/);
  });
});
