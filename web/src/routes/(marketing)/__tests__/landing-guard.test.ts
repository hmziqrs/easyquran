import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";

import { describe, expect, it } from "vite-plus/test";

/**
 * Landing guard (plan 05): the marketing surface draws colour ONLY through the
 * palette/hue tokens (§61 — no literal colours, no Tailwind default-palette
 * classes), and the rebuilt landing keeps its reader hrefs on the guarded
 * surahPathFor(ctx, …) path. Mockups win over sub-plan text; this file pins
 * the parts a screenshot cannot.
 *
 * Paths are cwd-relative (tests run from web/) — import.meta.url is not a
 * file: URL under the vite-plus test runner, so new URL(...) walking fails.
 */

function walk(dir: string, extension: string, out: string[] = []): string[] {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const child = join(dir, entry.name);
    if (entry.isDirectory()) walk(child, extension, out);
    else if (entry.name.endsWith(extension)) out.push(child);
  }
  return out;
}

function read(path: string): string {
  return readFileSync(path, "utf8");
}

const LANDING_PAGE = join("src/routes/(marketing)/+page.svelte");

describe("landing colour discipline (§61)", () => {
  const surfaces = [
    ...walk(join("src/routes/(marketing)"), ".svelte"),
    ...walk(join("src/lib/components/footer"), ".svelte"),
  ].filter((path) => !path.includes("__tests__"));

  it("guards a non-empty surface set", () => {
    expect(surfaces.length).toBeGreaterThan(8);
  });

  it("has no literal colours in marketing components", () => {
    for (const path of surfaces) {
      const source = read(path);
      expect(source, `${path}: oklch()`).not.toMatch(/oklch\(/i);
      expect(source, `${path}: rgb()/hsl()`).not.toMatch(/\b(?:rgb|rgba|hsl|hsla)\(/i);
      // 6+ hex digits — anchors like #main/#surahs cannot match.
      expect(source, `${path}: hex`).not.toMatch(/#[0-9a-fA-F]{6}\b/);
    }
  });

  it("has no Tailwind default-palette classes — semantic tokens only", () => {
    const PALETTE =
      /\b(?:bg|text|border|from|to|ring|fill|stroke)-(?:red|orange|amber|yellow|lime|green|emerald|teal|cyan|sky|blue|indigo|violet|purple|fuchsia|pink|rose|slate|gray|grey|zinc|neutral|stone)-\d{2,3}\b/;
    for (const path of surfaces) {
      expect(read(path), path).not.toMatch(PALETTE);
    }
  });
});

describe("landing href discipline (nav-guard complement)", () => {
  const landing = read(LANDING_PAGE);

  it("builds surah hrefs through surahPathFor with a route context", () => {
    expect(landing).toContain("surahPathFor(arabicCtx");
    expect(landing).toContain("readerHrefFor(locale, surahPathFor(arabicCtx, s))");
    expect(landing).toContain("publicHref(");
  });

  it("never hand-builds /app/ href strings", () => {
    expect(landing).not.toMatch(/href=\{?["']\/(?:en\/)?app\//);
  });

  it("keeps the strip and cards on the shared primitives", () => {
    expect(landing).toContain('from "$lib/components"');
    expect(landing).toContain("<Band");
    expect(landing).toContain("<MetricCard");
  });
});

describe("landing i18n keys", () => {
  function catalog(locale: string): Record<string, string> {
    return JSON.parse(read(join("messages", `${locale}.json`)));
  }

  const NEW_KEYS = [
    "landing_hero_title_full",
    "landing_hero_title_lead",
    "landing_hero_title_highlight",
    "landing_hero_title_tail",
    "landing_search_placeholder",
    "landing_search_button",
    "landing_often_opened",
    "landing_metric_surahs",
    "landing_metric_surahs_note",
    "landing_metric_juz",
    "landing_metric_juz_note",
    "landing_metric_pages",
    "landing_metric_pages_note",
    "landing_metric_bookmarks",
    "landing_metric_bookmarks_note",
    "landing_metric_bookmarks_empty",
    "landing_metric_yours",
    "landing_index_eyebrow",
    "landing_index_title",
    "landing_index_intro",
    "landing_index_see_all",
    "landing_why_eyebrow",
    "landing_why_title",
    "landing_why_intro",
    "landing_why_step1_title",
    "landing_why_step2_title",
    "landing_why_step3_title",
    "landing_closing_bismillah",
    "landing_closing_title",
    "landing_closing_intro",
    "landing_closing_cta",
    "landing_closing_note",
    "nav_surahs",
    "nav_juz",
    "nav_pages",
    "nav_about",
    "nav_header_search",
    "nav_start_reading",
  ];

  it("ships every new key in both locales with the bismillah verbatim", () => {
    const en = catalog("en");
    const ar = catalog("ar");
    for (const key of NEW_KEYS) {
      expect(en[key], `en: ${key}`).toBeTruthy();
      expect(ar[key], `ar: ${key}`).toBeTruthy();
    }
    expect(en.landing_closing_bismillah).toBe(ar.landing_closing_bismillah);
  });

  it("retired the pre-band keys so the catalogs carry no dead sections", () => {
    const en = catalog("en");
    for (const key of [
      "landing_badge",
      "landing_hero_title",
      "landing_primary_cta",
      "landing_today_eyebrow",
      "landing_today_title",
      "landing_today_intro",
      "landing_value_instant_title",
      "landing_coming",
    ]) {
      expect(en[key], key).toBeUndefined();
    }
  });
});
