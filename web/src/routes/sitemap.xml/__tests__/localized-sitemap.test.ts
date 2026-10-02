import { RANGE_COUNTS, RangeKind } from "$lib/data/quran-data";
import { HIZB_COUNT, RUB_COUNT } from "$lib/data/mushaf-divisions";
import { QURAN_DATA } from "$lib/server/quran-data";
import { describe, expect, it, vi } from "vite-plus/test";

import { GET } from "../+server";

vi.mock("$env/dynamic/public", () => ({ env: {} }));

const QURAN_DATA_SURAH_COUNT = QURAN_DATA.surahs.length;

function locs(xml: string): string[] {
  return [...xml.matchAll(/<loc>([^<]+)<\/loc>/g)].map((match) => match[1]!);
}

// The seven published marketing locs (home en+ar, five en-only pages); everything
// else in the sitemap is a reader URL.
const MARKETING_LOCS = new Set([
  "https://easyquran.fyi/",
  "https://easyquran.fyi/ar/",
  "https://easyquran.fyi/about",
  "https://easyquran.fyi/faq",
  "https://easyquran.fyi/contact",
  "https://easyquran.fyi/privacy",
  "https://easyquran.fyi/terms",
]);

function readerLocsOf(xml: string): string[] {
  return locs(xml).filter((href) => !MARKETING_LOCS.has(href));
}

// The sitemap fans out over every prerendered reader route, so a cold first call is slow under
// parallel suite load. The default 5s timeout flakes; the work itself is a few hundred ms warm.
describe("localized sitemap", { timeout: 30_000 }, () => {
  it("emits only published marketing locale-page pairs", async () => {
    const xml = await GET().text();
    const urls = locs(xml);

    expect(urls).toContain("https://easyquran.fyi/");
    expect(urls).toContain("https://easyquran.fyi/ar/");
    expect(urls).toContain("https://easyquran.fyi/about");
    expect(urls).not.toContain("https://easyquran.fyi/ar/about");
    expect(xml).toContain(
      '<xhtml:link rel="alternate" hreflang="ar" href="https://easyquran.fyi/ar/"/>',
    );
    expect(xml).toContain(
      '<xhtml:link rel="alternate" hreflang="x-default" href="https://easyquran.fyi/"/>',
    );
  });

  it("submits exactly the indexable reader set and never a dead spelling", async () => {
    const xml = await GET().text();
    const readerLocs = readerLocsOf(xml);

    const expectedReaderLocs =
      QURAN_DATA_SURAH_COUNT +
      RANGE_COUNTS[RangeKind.Page] +
      RANGE_COUNTS[RangeKind.Juz] +
      HIZB_COUNT +
      RUB_COUNT +
      3; // juz/surah/pages browse indexes
    expect(readerLocs).toHaveLength(expectedReaderLocs);
    // Scheme A: locs are unprefixed en forms; no legacy /app or /en spelling.
    expect(readerLocs.every((href) => href.startsWith("https://easyquran.fyi/"))).toBe(true);
    expect(readerLocs.some((href) => href.includes("/app"))).toBe(false);
    expect(readerLocs.some((href) => href.includes("/en/"))).toBe(false);
    expect(readerLocs).toContain("https://easyquran.fyi/juz");
    expect(readerLocs).toContain("https://easyquran.fyi/surah");
    expect(readerLocs).toContain("https://easyquran.fyi/pages");
  });

  it("carries no surah-local page locs and the full hizb/rub families", async () => {
    const xml = await GET().text();
    const readerLocs = readerLocsOf(xml);

    // One URL per surah: zero surah-local page URLs remain. The segment before
    // /page/ must not be the host, so the global /page/N family never matches.
    expect(
      readerLocs.some((href) => /\/(?!page\/)[^/]+\/page\/\d+$/.test(new URL(href).pathname)),
    ).toBe(false);
    for (let hizb = 1; hizb <= HIZB_COUNT; hizb += 8) {
      expect(readerLocs).toContain(`https://easyquran.fyi/hizb/${hizb}`);
    }
    expect(readerLocs).toContain(`https://easyquran.fyi/rub/${RUB_COUNT}`);
    expect(readerLocs).toContain("https://easyquran.fyi/juz/30");
  });

  it("carries the en/ar/x-default UI-locale hreflang triple on every reader url", async () => {
    const xml = await GET().text();
    const readerLocs = readerLocsOf(xml);
    expect(readerLocs.length).toBeGreaterThan(1000);
    for (const loc of readerLocs) {
      const block = xml
        .split("<url>")
        .find((candidate) => candidate.includes(`<loc>${loc}</loc>`));
      expect(block, `missing block for ${loc}`).toBeDefined();
      expect(block).toContain(`<xhtml:link rel="alternate" hreflang="en" href="${loc}"/>`);
      expect(block).toContain(
        `<xhtml:link rel="alternate" hreflang="ar" href="https://easyquran.fyi/ar${new URL(loc).pathname}"/>`,
      );
      expect(block).toContain(`<xhtml:link rel="alternate" hreflang="x-default" href="${loc}"/>`);
    }
    // Translated routes stay discovery-only (D12): never a <loc>, never an alternate.
    expect(xml).not.toMatch(/<loc>https:\/\/easyquran\.fyi\/[^<]*\/t\//);
    expect(xml).not.toMatch(/hreflang="ar" href="https:\/\/easyquran\.fyi\/ar\/[^"]*\/t\//);
    expect(xml).not.toMatch(/https:\/\/easyquran\.fyi\/en\/[^<"]+\.(?:md|txt)/);
    expect(xml).not.toMatch(/https:\/\/easyquran\.fyi\/app[^<"]*/);
  });
});
