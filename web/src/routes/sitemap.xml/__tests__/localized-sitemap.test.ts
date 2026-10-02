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

  it("submits exactly the indexable reader set and never the noindex app home", async () => {
    const xml = await GET().text();
    const readerLocs = locs(xml).filter((href) => href.includes("/app"));

    const expectedReaderLocs =
      QURAN_DATA_SURAH_COUNT +
      RANGE_COUNTS[RangeKind.Page] +
      RANGE_COUNTS[RangeKind.Juz] +
      HIZB_COUNT +
      RUB_COUNT +
      3; // juz/surah/pages browse indexes
    expect(readerLocs).toHaveLength(expectedReaderLocs);
    // /en/app is noindex — the sitemap stops submitting it (D11).
    expect(readerLocs).not.toContain("https://easyquran.fyi/en/app");
    expect(readerLocs).toContain("https://easyquran.fyi/en/app/juz");
    expect(readerLocs).toContain("https://easyquran.fyi/en/app/surah");
    expect(readerLocs).toContain("https://easyquran.fyi/en/app/pages");
    expect(readerLocs.every((href) => href.startsWith("https://easyquran.fyi/en/app"))).toBe(true);
    expect(readerLocs.some((href) => href.includes("/ar/app"))).toBe(false);
    expect(readerLocs.some((href) => href === "https://easyquran.fyi/app")).toBe(false);
  });

  it("carries no surah-local page locs and the full hizb/rub families", async () => {
    const xml = await GET().text();
    const readerLocs = locs(xml).filter((href) => href.includes("/app"));

    // One URL per surah: zero surah-local page URLs remain.
    expect(readerLocs.some((href) => /\/app\/[^/]+\/page\/\d+$/.test(href))).toBe(false);
    for (let hizb = 1; hizb <= HIZB_COUNT; hizb += 8) {
      expect(readerLocs).toContain(`https://easyquran.fyi/en/app/hizb/${hizb}`);
    }
    expect(readerLocs).toContain(`https://easyquran.fyi/en/app/rub/${RUB_COUNT}`);
    expect(readerLocs).toContain("https://easyquran.fyi/en/app/juz/30");
  });

  it("keeps reader <url> blocks free of xhtml:link alternates (D13)", async () => {
    const xml = await GET().text();
    const readerBlocks = xml.split("<url>").filter((block) => block.includes("/en/app"));
    expect(readerBlocks.length).toBeGreaterThan(0);
    for (const block of readerBlocks) {
      expect(block.includes("xhtml:link")).toBe(false);
    }
    // Translated routes stay discovery-only (D12): never a <loc>, never an alternate.
    expect(xml).not.toMatch(/<loc>https:\/\/easyquran\.fyi\/en\/app\/[^<]*\/t\//);
    expect(xml).not.toMatch(/href="https:\/\/easyquran\.fyi\/ar\/app\/[^"]*\/t\//);
    expect(xml).not.toMatch(/https:\/\/easyquran\.fyi\/en\/app[^<"]+\.(?:md|txt)/);
  });
});
