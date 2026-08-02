<script lang="ts">
  import { onMount, tick } from "svelte";
  import { goto } from "$app/navigation";
  import { resolve } from "$app/paths";
  import { page } from "$app/state";
  import { Seo } from "$lib/components";
  import {
    surahAyahPath,
    surahLocalPagePath,
    type SurahRouteData,
  } from "$lib/data/quran";
  import { loadQuranData } from "$lib/data/quran-data-client";
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
  const canonicalPath = $derived(surahLocalPagePath(surah, activeLocalPage));
  const pageSuffix = $derived(
    data.pageData.pageCount > 1
      ? ` — Page ${activeLocalPage} of ${data.pageData.pageCount}`
      : "",
  );
  const seoTitle = $derived(
    `Surah ${surah.num}, ${surah.name}${pageSuffix} · EasyQuran`,
  );
  const seoDescription = $derived(
    `Read Surah ${surah.name} (${surah.arabic}), page ${activeLocalPage} of ${data.pageData.pageCount}, ayahs ${activePage.page.startAyah}–${activePage.page.endAyah}, in the Uthmani script.`,
  );
  const chapterLd = $derived([
    {
      "@context": "https://schema.org",
      "@type": "Chapter",
      name: `Surah ${surah.name}`,
      alternateName: surah.arabic,
      position: surah.num,
      inLanguage: "ar",
      isPartOf: { "@type": "Book", name: "The Quran", inLanguage: "ar" },
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

  async function revealRequestedAyah(): Promise<void> {
    const ayah = requestedAyah();
    if (!ayah) return;
    anchorScrolling = true;
    try {
      const quranData = await loadQuranData();
      const targetPage = quranData.surahLocalPageForAyah(surah.num, ayah);
      if (!targetPage) return;
      const targetHref = resolve(surahAyahPath(surah, targetPage.localPage, ayah));
      if (
        targetPage.localPage !== data.pageData.page.localPage ||
        page.url.searchParams.has("verse")
      ) {
        await goto(targetHref, { replaceState: true, keepFocus: true, noScroll: true });
        return;
      }
      await tick();
      await document.fonts.ready;
      await nextFrame();
      await nextFrame();
      const row = document.getElementById(`ayah-${surah.num}-${ayah}`);
      const target = row?.querySelector<HTMLElement>("[data-verse-anchor]") ?? row;
      target?.scrollIntoView({ behavior: "auto", block: "center" });
      reader.markRead(surah.num, ayah);
      await nextFrame();
    } finally {
      anchorScrolling = false;
    }
  }

  onMount(() => {
    reader.setCurrent(surah.num);
    void revealRequestedAyah();
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
