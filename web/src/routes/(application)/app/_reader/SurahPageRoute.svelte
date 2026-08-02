<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/state";
  import { Seo } from "$lib/components";
  import { surahLocalPagePath, type SurahRouteData } from "$lib/data/quran";
  import { reader } from "$lib/stores/reader.svelte";
  import ReaderShell from "./ReaderShell.svelte";
  import Results from "./Results.svelte";
  import SurahReader from "./SurahReader.svelte";

  let { data }: { data: SurahRouteData } = $props();
  const surah = $derived(data.pageData.surah);
  let scrolledPage = $state<typeof data.pageData | null>(null);
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

  function revealRequestedAyah(): void {
    const legacyAyah = page.url.searchParams.get("verse");
    const id = page.url.hash.slice(1) || (legacyAyah ? `ayah-${surah.num}-${legacyAyah}` : "");
    if (!id) return;
    document.getElementById(id)?.scrollIntoView({ behavior: "smooth", block: "center" });
  }

  onMount(() => {
    reader.setCurrent(surah.num);
    requestAnimationFrame(revealRequestedAyah);
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
    <span dir="rtl" class="ml-auto font-arabic text-base text-fg-3">{surah.arabic}</span>
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
      onVisiblePage={(pageData) => (scrolledPage = pageData)}
    />
  {/if}
</ReaderShell>
