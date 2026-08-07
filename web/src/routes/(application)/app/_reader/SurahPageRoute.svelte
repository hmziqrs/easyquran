<script lang="ts">
  import { onMount, tick } from "svelte";
  import { goto, replaceState } from "$app/navigation";
  import { resolve } from "$app/paths";
  import { page } from "$app/state";
  import { Seo } from "$lib/components";
  import {
    QuranScript,
    surahAyahPathFor,
    surahLocalPagePathFor,
    surahRouteContext,
    translationSegmentsFromId,
    type SurahRouteData,
  } from "$lib/data/quran";
  import { loadQuranData } from "$lib/data/quran-data-client";
  import { trackReaderView } from "$lib/quran/track-view.svelte";
  import { reader } from "$lib/stores/reader.svelte";
  import ReaderShell from "./ReaderShell.svelte";
  import Results from "./Results.svelte";
  import SurahReader from "./SurahReader.svelte";

  let { data }: { data: SurahRouteData } = $props();
  const surah = $derived(data.pageData.surah);
  let scrolledPage = $state<typeof data.pageData | null>(null);
  let anchorScrolling = $state(false);
  const activePage = $derived(scrolledPage ?? data.pageData);
  const activeLocalPage = $derived(activePage.page.localPage);
  const normalization = $derived(data.pageData.normalization);
  const routeContext = $derived(surahRouteContext(normalization.sourceId));
  const canonicalPath = $derived(surahLocalPagePathFor(routeContext, surah, activeLocalPage));
  const pageSuffix = $derived(
    data.pageData.pageCount > 1
      ? ` — Page ${activeLocalPage} of ${data.pageData.pageCount}`
      : "",
  );
  const seoTitle = $derived(
    `Surah ${surah.num}, ${surah.name}${pageSuffix} · EasyQuran`,
  );
  const isTranslation = $derived(normalization.script === QuranScript.Translation);
  const contentLanguage = $derived(
    isTranslation ? translationSegmentsFromId(normalization.sourceId).lang : "ar",
  );
  const seoDescription = $derived(
    `Read Surah ${surah.name} (${surah.arabic}), page ${activeLocalPage} of ${data.pageData.pageCount}, ayahs ${activePage.page.startAyah}–${activePage.page.endAyah}${
      isTranslation ? "" : ", in the Uthmani script."
    }.`,
  );
  const chapterLd = $derived([
    {
      "@context": "https://schema.org",
      "@type": "Chapter",
      name: `Surah ${surah.name}`,
      alternateName: surah.arabic,
      position: surah.num,
      inLanguage: contentLanguage,
      isPartOf: { "@type": "Book", name: "The Quran", inLanguage: contentLanguage },
    },
  ]);

  function requestedAyah(): number | null {
    const legacy = page.url.searchParams.get("verse");
    const hash = new RegExp(`^#ayah-${surah.num}-(\\d+)$`).exec(page.url.hash)?.[1];
    const value = Number(hash ?? legacy);
    return Number.isSafeInteger(value) && value >= 1 && value <= surah.ayahCount ? value : null;
  }

  function nextFrame(): Promise<void> {
    return new Promise((resolveFrame) => requestAnimationFrame(() => resolveFrame()));
  }

  async function ayahRow(ayah: number): Promise<HTMLElement | null> {
    const id = `ayah-${surah.num}-${ayah}`;
    await tick();
    await document.fonts.ready;
    for (let attempt = 0; attempt < 4; attempt += 1) {
      await nextFrame();
      const row = document.getElementById(id);
      if (row) return row;
    }
    return null;
  }

  async function revealRequestedAyah(ayah: number): Promise<void> {
    anchorScrolling = true;
    try {
      const quranData = await loadQuranData();
      const targetPage = quranData.surahLocalPageForAyah(surah.num, ayah);
      if (!targetPage) return;
      const targetHref = resolve(surahAyahPathFor(routeContext, surah, targetPage.localPage, ayah));
      if (targetPage.localPage !== data.pageData.page.localPage) {
        await goto(targetHref, { replaceState: true, keepFocus: true, noScroll: true });
        return;
      }
      if (page.url.href !== new URL(targetHref, page.url).href) {
        replaceState(targetHref, page.state);
      }
      const row = await ayahRow(ayah);
      if (!row) {
        await goto(targetHref, { replaceState: true, keepFocus: true, noScroll: true });
        return;
      }
      const target = row.querySelector<HTMLElement>("[data-verse-anchor]") ?? row;
      const start = performance.now();
      let lastHeight = -1;
      let stableFrames = 0;
      for (;;) {
        target.scrollIntoView({ behavior: "auto", block: "center" });
        const height = document.documentElement.scrollHeight;
        stableFrames = height === lastHeight ? stableFrames + 1 : 0;
        lastHeight = height;
        const elapsed = performance.now() - start;
        if (elapsed > 700) break;
        if (stableFrames >= 3 && elapsed >= 320) break;
        await nextFrame();
      }
      reader.markRead(surah.num, ayah, normalization.sourceId);
      await nextFrame();
    } finally {
      anchorScrolling = false;
    }
  }

  let revealedAyah: number | null = null;

  onMount(() => {
    reader.setCurrent(surah.num);
  });

  const viewKey = $derived(`${normalization.sourceId}:${surah.num}`);
  trackReaderView({ key: () => viewKey, sourceId: () => normalization.sourceId });

  $effect(() => {
    const ayah = requestedAyah();
    if (ayah === null) {
      revealedAyah = null;
      return;
    }
    if (revealedAyah === ayah) return;
    revealedAyah = ayah;
    void revealRequestedAyah(ayah);
  });
</script>

<Seo
  path={canonicalPath}
  title={seoTitle}
  description={seoDescription}
  extraLd={chapterLd}
  includeTextVariants={false}
/>

<ReaderShell>
  {#snippet header()}
    <span class="text-sm font-medium text-fg-2">
      {surah.num}. {surah.name} · Page {activeLocalPage}/{data.pageData.pageCount}
    </span>
    <span dir="rtl" lang="ar" class="ml-auto font-arabic text-base text-fg-3">
      {surah.arabic}
    </span>
  {/snippet}

  {#if reader.hasQuery}
    <Results />
  {:else}
    <SurahReader
      initial={data.pageData}
      previousPage={data.previousPage}
      nextPage={data.nextPage}
      previousSurah={data.previousSurah}
      nextSurah={data.nextSurah}
      {anchorScrolling}
      onVisiblePage={(pageData) => (scrolledPage = pageData)}
    />
  {/if}
</ReaderShell>
