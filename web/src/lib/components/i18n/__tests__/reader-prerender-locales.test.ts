import {
  quranHrefForPrerenderEntry,
  readerPrerenderEntries,
  readerPrerenderHrefs,
} from "$lib/components/i18n/reader-prerender.server";
import ReaderPrerenderLinks from "$lib/components/i18n/ReaderPrerenderLinks.svelte";
import { RANGE_COUNTS, RangeKind } from "$lib/data/quran-data";
import { HIZB_COUNT, RUB_COUNT } from "$lib/data/mushaf-divisions";
import { SUPPORTED_UI_LOCALES } from "$lib/i18n/locales";
import { readerHrefFor } from "$lib/i18n/reader";
import { readerEntryPath } from "$lib/i18n/seo";
import { QURAN_DATA } from "$lib/server/quran-data";
import { mount, unmount } from "svelte";
import { describe, expect, it, vi } from "vite-plus/test";

vi.mock("$env/dynamic/public", () => ({ env: {} }));

const READER_ENTRY_COUNT =
  QURAN_DATA.surahs.length +
  RANGE_COUNTS[RangeKind.Page] +
  RANGE_COUNTS[RangeKind.Juz] +
  HIZB_COUNT +
  RUB_COUNT;
const INDEX_COUNT = 4; // home + juz/surah/pages indexes, per UI locale

describe("localized reader prerender discovery", () => {
  it("matches every existing Arabic-source entry exactly once", () => {
    const entries = readerPrerenderEntries(QURAN_DATA);
    expect(entries.filter((entry) => entry.kind === "surah")).toHaveLength(114);
    // The surah-local page scheme is gone (its kind no longer exists); hizb/rub joined.
    const kinds = new Set<string>(entries.map((entry) => entry.kind));
    expect(kinds.has("surah-local-page")).toBe(false);
    expect(entries.filter((entry) => entry.kind === "global-page")).toHaveLength(
      RANGE_COUNTS[RangeKind.Page],
    );
    expect(entries.filter((entry) => entry.kind === "juz")).toHaveLength(RANGE_COUNTS[RangeKind.Juz]);
    expect(entries.filter((entry) => entry.kind === "hizb")).toHaveLength(HIZB_COUNT);
    expect(entries.filter((entry) => entry.kind === "rub")).toHaveLength(RUB_COUNT);
    expect(HIZB_COUNT).toBe(60);
    expect(RUB_COUNT).toBe(240);
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
    expect(hrefs.filter((href) => href.startsWith("/en/app"))).toHaveLength(
      READER_ENTRY_COUNT + INDEX_COUNT,
    );
    expect(hrefs.filter((href) => href.startsWith("/ar/app"))).toHaveLength(
      READER_ENTRY_COUNT + INDEX_COUNT,
    );
    const entryHrefs = new Set([
      "/en/app",
      "/en/app/juz",
      "/en/app/surah",
      "/en/app/pages",
      "/ar/app",
      "/ar/app/juz",
      "/ar/app/surah",
      "/ar/app/pages",
    ]);
    expect(hrefs.filter((href) => entryHrefs.has(href))).toHaveLength(8);
    expect(hrefs.every((href) => !href.includes("/t/"))).toBe(true);
    expect(hrefs.every((href) => !href.includes("/page/") || /\/app\/page\/\d+$/.test(href))).toBe(
      true,
    );
    expect(hrefs).toContain("/en/app/hizb/1");
    expect(hrefs).toContain("/ar/app/rub/240");
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
    expect(container?.querySelector('a[href="/ar/app/juz"]')).not.toBeNull();
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
    ).toBe("/app/ar-rum/t/ms/basmeih");

    const rub = readerPrerenderEntries(QURAN_DATA).find((entry) => entry.kind === "rub");
    expect(rub).toBeDefined();
    expect(
      quranHrefForPrerenderEntry(rub!, {
        kind: "translation",
        lang: "ms",
        translator: "basmeih",
      }),
    ).toBe("/app/t/ms/basmeih/rub/1");
  });
});
