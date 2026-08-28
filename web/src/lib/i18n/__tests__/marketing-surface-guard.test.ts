import { readFileSync } from "node:fs";

import { describe, expect, it } from "vite-plus/test";

import { resolveLandingCopy } from "../landing-copy";
import type { LandingResolvedCopy } from "../marketing-copy";

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
    const marketingTweaks = source(
      "../../../routes/(marketing)/_components/MarketingTweaks.svelte",
    );
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

  it("routes reader logo links to localized marketing home", () => {
    const readerLayout = source("../../../routes/(application)/app/+layout.svelte");

    expect(readerLayout).toContain("brandHomeHref={marketingHomeHref(copy.locale)}");
    expect(readerLayout).not.toContain("brandHomeHref={currentReaderHref}");
  });
});

describe("landing surface (plan 05)", () => {
  /** Every band's copy, flattened through the typed shape — new sections join this list. */
  function landingStrings(copy: LandingResolvedCopy): string[] {
    return [
      copy.heroTitleFull,
      copy.heroTitleLead,
      copy.heroTitleHighlight,
      copy.heroTitleTail,
      copy.heroIntro,
      copy.secondaryCta,
      copy.searchLabel,
      copy.searchPlaceholder,
      copy.searchButton,
      copy.oftenOpened,
      copy.metricSurahs,
      copy.metricSurahsNote,
      copy.metricJuz,
      copy.metricJuzNote,
      copy.metricPages,
      copy.metricPagesNote,
      copy.metricBookmarks,
      copy.metricBookmarksNote,
      copy.metricBookmarksEmpty,
      copy.metricYours,
      copy.indexEyebrow,
      copy.indexTitle,
      copy.indexIntro,
      copy.indexSeeAll,
      copy.whyEyebrow,
      copy.whyTitle,
      copy.whyIntro,
      ...copy.steps.flatMap((step) => [step.title, step.body]),
      copy.roadmapEyebrow,
      copy.roadmapTitle,
      copy.roadmapIntro,
      ...copy.roadmap.flatMap((item) => [item.title, item.body]),
      copy.closingBismillah,
      copy.closingTitle,
      copy.closingIntro,
      copy.closingCta,
      copy.closingNote,
    ];
  }

  it("resolves every new band's copy in both locales", () => {
    for (const locale of ["en", "ar"] as const) {
      const copy = resolveLandingCopy(locale);
      const strings = landingStrings(copy);
      // New sections resolve to real text in BOTH catalogs (i18n:check covers
      // parity; this catches a key that resolves to "" and renders nothing).
      expect(copy.metricSurahs.length, locale).toBeGreaterThan(0);
      expect(copy.metricBookmarksEmpty.length, locale).toBeGreaterThan(0);
      expect(copy.searchPlaceholder.length, locale).toBeGreaterThan(0);
      expect(copy.indexSeeAll.length, locale).toBeGreaterThan(0);
      expect(copy.closingBismillah.length, locale).toBeGreaterThan(0);
      expect(copy.steps, locale).toHaveLength(3);
      expect(copy.roadmap, locale).toHaveLength(4);
      expect(strings.every((s) => s.trim().length > 0), locale).toBe(true);
    }
  });

  it("carries hue slots as numbers only — presentation never lives in the copy layer", () => {
    const TAILWIND_CLASS = /\b(?:bg|text)-[a-z][a-z0-9-]*\b/;

    for (const locale of ["en", "ar"] as const) {
      const copy = resolveLandingCopy(locale);
      for (const leaf of landingStrings(copy)) {
        expect(leaf, `${locale}: ${leaf}`).not.toMatch(TAILWIND_CLASS);
      }
      for (const entry of [...copy.steps, ...copy.roadmap]) {
        expect(Number.isInteger(entry.hue), `${locale}: ${entry.id}`).toBe(true);
        expect(entry.hue, `${locale}: ${entry.id}`).toBeGreaterThanOrEqual(1);
        expect(entry.hue, `${locale}: ${entry.id}`).toBeLessThanOrEqual(4);
      }
    }
  });

  it("keeps the hero search a real control on the TanStack hotkey path", () => {
    const landing = source("../../../routes/(marketing)/+page.svelte");

    // Real form submitting to the search route.
    expect(landing).toContain('method="GET"');
    expect(landing).toContain('name="q"');
    expect(landing).toContain('for="hero-search"');
    // Hotkeys: dynamic import of the shared wrapper, register + unregister, IME guard.
    expect(landing).toContain('import("$lib/hotkeys.svelte")');
    expect(landing).toContain("registerHotkey");
    expect(landing).toContain(".unregister()");
    expect(landing).toContain("event.isComposing");
    // No hand-rolled key listeners for app shortcuts.
    expect(landing).not.toContain("onkeydown");
    expect(landing).not.toContain("addEventListener");
  });

  it("draws the metric strip gapless and edge to edge with real bookmark data", () => {
    const landing = source("../../../routes/(marketing)/+page.svelte");

    expect(landing).toContain('contentClass="px-0 md:px-0 lg:px-0 xl:px-0"');
    expect(landing).toContain("md:grid-cols-2 lg:grid-cols-4");
    expect(landing).toContain("gap-0");
    expect((landing.match(/<MetricCard /g) ?? []).length).toBeGreaterThanOrEqual(4);
    // Real data only: the store arrives behind a dynamic import, never fabricated samples.
    expect(landing).toContain('import("$lib/bookmarks/store.svelte")');
    expect(landing).not.toMatch(/bookmark(s)?[\s\S]{0,40}example/i);
  });

  it("renders the board header and footer from the marketing layout", () => {
    const layout = source("../../../routes/(marketing)/+layout.svelte");

    expect(layout).toContain("MarketingHeader");
    expect(layout).not.toContain("MarketingNav");
    expect(layout).toContain("MarketingFooter");
  });
});
