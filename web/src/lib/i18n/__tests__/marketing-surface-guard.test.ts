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

    expect(copySource).not.toContain("#lib/paraglide");
    expect(copySource).not.toContain("#lib/i18n/m/");
    expect(copySource).not.toMatch(/messages\/(?:en|ar)\.json/);
  });

  it("resolves each namespace from its own generated barrel", () => {
    const chrome = source("../chrome-copy.ts");
    const appearance = source("../appearance-copy.ts");
    const landing = source("../landing-copy.ts");

    expect(chrome).toContain('from "#lib/i18n/m/chrome.js"');
    expect(chrome).not.toContain('from "#lib/i18n/m/landing.js"');
    expect(chrome).not.toContain('from "#lib/i18n/m/appearance.js"');
    expect(appearance).toContain('from "#lib/i18n/m/appearance.js"');
    expect(appearance).not.toContain('from "#lib/i18n/m/chrome.js"');
    expect(landing).toContain('from "#lib/i18n/m/landing.js"');
    expect(landing).not.toContain('from "#lib/i18n/m/chrome.js"');
    for (const module of [chrome, appearance, landing]) {
      expect(module).toContain("{ locale }");
    }
  });

  it("loads both appearance panels lazily", () => {
    const marketingTweaks = source(
      "../../../routes/(marketing)/_components/MarketingTweaks.svelte",
    );
    const readerLayout = source("../../../routes/(application)/+layout.svelte");
    const tweaks = source("../../components/tweaks/Tweaks.svelte");

    expect(marketingTweaks).toContain('await import("#lib/i18n/appearance-copy.js")');
    expect(readerLayout).toContain('await import("#lib/i18n/reader-settings-copy.js")');
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

    expect(shared).not.toContain("#lib/paraglide");
    expect(shared).not.toMatch(/messages\/(?:en|ar)\.json/);
  });

  it("keeps the footer free of the retired Quran quotation", () => {
    const footer = source("../../components/footer/Footer.svelte");

    expect(footer).not.toContain("وَنَزَّلْنَا");
    expect(footer).not.toContain('lang="ar"');
  });

  it("routes landing reader links through the reader home helper", () => {
    const landing = source("../../../routes/(marketing)/+page.svelte");

    expect(landing).toContain("readerHrefFor(locale, surahPathFor(arabicCtx, s))");
    expect(landing).not.toMatch(/href=["']\/(?:en|ar)?\/?app/);
  });

  it("routes reader logo links to localized marketing home", () => {
    const readerLayout = source("../../../routes/(application)/+layout.svelte");

    expect(readerLayout).toContain("brandHomeHref={marketingHomeHref(copy.locale)}");
    expect(readerLayout).not.toContain("brandHomeHref={currentReaderHref}");
  });
});

describe("landing surface (index-first home)", () => {
  /** Every string the home renders, flattened through the typed shape. */
  function landingStrings(copy: LandingResolvedCopy): string[] {
    return [
      copy.heroTitleFull,
      copy.bismillah,
      copy.searchLabel,
      copy.searchPlaceholder,
      copy.searchButton,
      copy.oftenOpened,
      copy.indexLabel,
      copy.indexSurahs,
      copy.indexJuz,
      copy.indexPages,
      copy.indexYours,
      copy.ayahCount(7),
    ];
  }

  it("resolves every string in both locales", () => {
    for (const locale of ["en", "ar"] as const) {
      // i18n:check covers parity; this catches a key that resolves to "" and renders nothing.
      expect(
        landingStrings(resolveLandingCopy(locale)).every((s) => s.trim().length > 0),
        locale,
      ).toBe(true);
    }
  });

  it("keeps presentation out of the copy layer", () => {
    const TAILWIND_CLASS = /\b(?:bg|text)-[a-z][a-z0-9-]*\b/;

    for (const locale of ["en", "ar"] as const) {
      for (const leaf of landingStrings(resolveLandingCopy(locale))) {
        expect(leaf, `${locale}: ${leaf}`).not.toMatch(TAILWIND_CLASS);
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
    expect(landing).toContain('import("#lib/hotkeys.svelte.js")');
    expect(landing).toContain("registerHotkey");
    expect(landing).toContain(".unregister()");
    expect(landing).toContain("event.isComposing");
    // No hand-rolled key listeners for app shortcuts.
    expect(landing).not.toContain("onkeydown");
    expect(landing).not.toContain("addEventListener");
  });

  it("is the index, not a pitch: no marketing bands, same frame as the header", () => {
    const landing = source("../../../routes/(marketing)/+page.svelte");

    expect(landing).toContain("<Container");
    for (const band of ["<Band", "<MetricCard", "roadmap", "closing", "whyTitle"]) {
      expect(landing, band).not.toContain(band);
    }
  });

  it("renders the shared Nav and the footer from the marketing layout", () => {
    const layout = source("../../../routes/(marketing)/+layout.svelte");
    const nav = source("../../components/nav/Nav.svelte");

    expect(layout).toContain("<Nav");
    expect(layout).not.toContain("MarketingHeader");
    expect(layout).toContain("MarketingFooter");
    // The interface language is switched from Settings only — never from the header.
    expect(nav).not.toContain("hreflang");
    expect(nav).not.toContain("localeLinks");
  });
});
