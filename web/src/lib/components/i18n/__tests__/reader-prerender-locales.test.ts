import { mount, unmount } from "svelte";
import { describe, expect, it, vi } from "vite-plus/test";

import {
  quranHrefForPrerenderEntry,
  readerPrerenderEntries,
  readerPrerenderHrefs,
} from "#lib/components/i18n/reader-prerender.server.js";
import ReaderPrerenderLinks from "#lib/components/i18n/ReaderPrerenderLinks.svelte";
import { HIZB_COUNT } from "#lib/data/mushaf-divisions.js";
import { RANGE_COUNTS, RangeKind } from "#lib/data/quran-data.js";
import { SUPPORTED_UI_LOCALES } from "#lib/i18n/locales.js";
import { readerHrefFor } from "#lib/i18n/reader.js";
import { readerEntryPath } from "#lib/i18n/seo.js";
import { QURAN_DATA } from "#lib/server/quran-data.js";

vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: undefined,
  PUBLIC_QURAN_API_BASE: undefined,
  PUBLIC_ENV: undefined,
  PUBLIC_FCM_VAPID_KEY: undefined,
}));

const READER_ENTRY_COUNT =
  QURAN_DATA.surahs.length +
  RANGE_COUNTS[RangeKind.Page] +
  RANGE_COUNTS[RangeKind.Juz] +
  HIZB_COUNT;
const INDEX_COUNT = 3; // juz/surah/pages indexes, per UI locale — the /app hub is gone

describe("localized reader prerender discovery", () => {
  it("matches every existing Arabic-source entry exactly once", () => {
    const entries = readerPrerenderEntries(QURAN_DATA);
    expect(entries.filter((entry) => entry.kind === "surah")).toHaveLength(114);
    // The surah-local page scheme and rubʿ URLs are gone (their kinds no longer exist).
    const kinds = new Set<string>(entries.map((entry) => entry.kind));
    expect(kinds.has("surah-local-page")).toBe(false);
    expect(kinds.has("rub")).toBe(false);
    expect(entries.filter((entry) => entry.kind === "global-page")).toHaveLength(
      RANGE_COUNTS[RangeKind.Page],
    );
    expect(entries.filter((entry) => entry.kind === "juz")).toHaveLength(
      RANGE_COUNTS[RangeKind.Juz],
    );
    expect(entries.filter((entry) => entry.kind === "hizb")).toHaveLength(HIZB_COUNT);
    expect(HIZB_COUNT).toBe(60);
    expect(entries).toHaveLength(READER_ENTRY_COUNT);
  });

  it("fans Arabic-source entries across en/ar and never discovers translations", () => {
    const hrefs = readerPrerenderHrefs(
      QURAN_DATA,
      SUPPORTED_UI_LOCALES,
      readerHrefFor,
      readerEntryPath,
    );
    const total = (READER_ENTRY_COUNT + INDEX_COUNT) * SUPPORTED_UI_LOCALES.length;
    expect(hrefs).toHaveLength(total);
    expect(new Set(hrefs).size).toBe(total);
    // Scheme A: en hrefs are unprefixed, ar hrefs carry /ar.
    expect(hrefs.filter((href) => href.startsWith("/en/"))).toHaveLength(0);
    expect(hrefs.filter((href) => !href.startsWith("/ar/"))).toHaveLength(
      READER_ENTRY_COUNT + INDEX_COUNT,
    );
    expect(hrefs.filter((href) => href.startsWith("/ar/"))).toHaveLength(
      READER_ENTRY_COUNT + INDEX_COUNT,
    );
    expect(hrefs.every((href) => !href.includes("/app"))).toBe(true);
    const entryHrefs = new Set(["/juz", "/surah", "/pages", "/ar/juz", "/ar/surah", "/ar/pages"]);
    expect(hrefs.filter((href) => entryHrefs.has(href))).toHaveLength(6);
    expect(hrefs.every((href) => !href.includes("/t/"))).toBe(true);
    expect(hrefs.every((href) => !href.includes("/page/") || /\/page\/\d+$/.test(href))).toBe(true);
    expect(hrefs).toContain("/hizb/1");
    expect(hrefs).toContain("/ar/hizb/60");
    expect(hrefs.every((href) => !href.includes("/rub/"))).toBe(true);
    expect(hrefs.every((href) => !href.endsWith(".md") && !href.endsWith(".txt"))).toBe(true);
  });

  it("renders every build-discovery href as a hidden crawler anchor", async () => {
    const hrefs = readerPrerenderHrefs(
      QURAN_DATA,
      SUPPORTED_UI_LOCALES,
      readerHrefFor,
      readerEntryPath,
    );
    const target = document.createElement("div");
    document.body.append(target);
    const component = mount(ReaderPrerenderLinks, {
      target,
      props: { hrefs, enabled: true },
    });

    const container = target.querySelector<HTMLElement>("[data-reader-prerender-links]");
    expect(container?.hidden).toBe(true);
    expect(container?.querySelectorAll("a")).toHaveLength(hrefs.length);
    expect(container?.querySelector('a[href="/ar/juz"]')).not.toBeNull();
    expect(container?.querySelector('a[href*="/t/"]')).toBeNull();

    await unmount(component);
    target.remove();
  });

  it("can map the same descriptor to a translation without losing source context", () => {
    const surah = readerPrerenderEntries(QURAN_DATA).find(
      (entry) => entry.kind === "surah" && entry.surah.slug === "ar-rum",
    );
    expect(surah).toBeDefined();
    expect(
      quranHrefForPrerenderEntry(surah!, {
        kind: "translation",
        lang: "ms",
        translator: "basmeih",
      }),
    ).toBe("/ar-rum/t/ms/basmeih");

    const hizb = readerPrerenderEntries(QURAN_DATA).find((entry) => entry.kind === "hizb");
    expect(hizb).toBeDefined();
    expect(
      quranHrefForPrerenderEntry(hizb!, {
        kind: "translation",
        lang: "ms",
        translator: "basmeih",
      }),
    ).toBe("/t/ms/basmeih/hizb/1");
  });
});
