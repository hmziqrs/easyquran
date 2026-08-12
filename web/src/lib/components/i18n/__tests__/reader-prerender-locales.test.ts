import { describe, expect, it, vi } from "vite-plus/test";
import {
  quranHrefForPrerenderEntry,
  readerPrerenderEntries,
  readerPrerenderHrefs,
} from "$lib/components/i18n/reader-prerender.server";
import { SUPPORTED_UI_LOCALES } from "$lib/i18n/locales";
import { readerHrefFor } from "$lib/i18n/reader";
import { readerEntryPath } from "$lib/i18n/seo";
import { QURAN_DATA } from "$lib/server/quran-data";
import { mount, unmount } from "svelte";
import ReaderPrerenderLinks from "$lib/components/i18n/ReaderPrerenderLinks.svelte";

vi.mock("$env/dynamic/public", () => ({ env: {} }));

describe("localized reader prerender discovery", () => {
  it("matches every existing Arabic-source entry exactly once", () => {
    const entries = readerPrerenderEntries(QURAN_DATA);
    expect(entries.filter((entry) => entry.kind === "surah")).toHaveLength(114);
    expect(entries.filter((entry) => entry.kind === "surah-local-page")).toHaveLength(548);
    expect(entries.filter((entry) => entry.kind === "global-page")).toHaveLength(604);
    expect(entries.filter((entry) => entry.kind === "juz")).toHaveLength(30);
    expect(entries).toHaveLength(1_296);
  });

  it("fans 1,296 Arabic-source entries across en/ar and never discovers translations", () => {
    const hrefs = readerPrerenderHrefs(
      QURAN_DATA,
      SUPPORTED_UI_LOCALES,
      readerHrefFor,
      readerEntryPath,
    );
    expect(hrefs).toHaveLength(2_596);
    expect(new Set(hrefs).size).toBe(2_596);
    expect(hrefs.filter((href) => href.startsWith("/en/app"))).toHaveLength(1_298);
    expect(hrefs.filter((href) => href.startsWith("/ar/app"))).toHaveLength(1_298);
    const entryHrefs = new Set(["/en/app", "/en/app/juz", "/ar/app", "/ar/app/juz"]);
    expect(hrefs.filter((href) => entryHrefs.has(href))).toHaveLength(4);
    expect(hrefs.filter((href) => !entryHrefs.has(href))).toHaveLength(2_592);
    expect(hrefs.every((href) => !href.includes("/t/"))).toBe(true);
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
    expect(container?.querySelectorAll("a")).toHaveLength(2_596);
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
  });
});
