<script lang="ts">
  import { onMount, tick } from "svelte";
  import { replaceState } from "$app/navigation";
  import { page } from "$app/state";
  import { SITE } from "#lib/config/site.js";
  import { Seo } from "#lib/components/index.js";
  import {
    QuranScript,
    surahAyahPathFor,
    surahPathFor,
    surahRouteContext,
    translationSegmentsFromId,
    type SurahRouteData,
  } from "#lib/data/quran.js";
  import { loadQuranData, peekQuranData } from "#lib/data/quran-data-client.js";
  import { positionForGlobal } from "#lib/data/mushaf-divisions.js";
  import { getReaderUiCopy } from "#lib/i18n/reader-copy.js";
  import { readerHrefFor } from "#lib/i18n/reader.js";
  import { publicHref } from "#lib/i18n/public-href.js";
  import { trackReaderView } from "#lib/quran/track-view.svelte.js";
  import { reader } from "#lib/stores/reader.svelte.js";
  import { withModeParam } from "#lib/reader/mode-param.js";
  import ReaderShell from "./ReaderShell.svelte";
  import Results from "./Results.svelte";
  import SurahReader from "./SurahReader.svelte";

  let { data }: { data: SurahRouteData } = $props();
  const copy = getReaderUiCopy();
  const surah = $derived(data.pageData.surah);
  let scrolledPage = $state<typeof data.pageData | null>(null);
  let anchorScrolling = $state(false);
  let readerView: SurahReader | null = $state(null);
  const activePage = $derived(scrolledPage ?? data.pageData);
  const normalization = $derived(data.pageData.normalization);
  const routeContext = $derived(surahRouteContext(normalization.sourceId));
  // One canonical, one title, one description per surah — never scroll-dependent.
  const canonicalPath = $derived(surahPathFor(routeContext, surah));
  const canonicalPublicPath = $derived(readerHrefFor("en", canonicalPath));
  const currentPublicPath = $derived(readerHrefFor(copy.locale, canonicalPath));
  const seoTitle = $derived(copy.seo.surahTitle(surah.num, surah.name));
  const isTranslation = $derived(normalization.script === QuranScript.Translation);
  const contentLanguage = $derived(
    isTranslation ? translationSegmentsFromId(normalization.sourceId).lang : "ar",
  );
  const seoDescription = $derived(
    isTranslation
      ? copy.seo.surahDescriptionTranslation(surah.name, surah.arabic)
      : copy.seo.surahDescriptionUthmani(surah.name, surah.arabic),
  );
  const translationPending = $derived(isTranslation && data.pageData.ayahs.length === 0);
  const chapterLd = $derived([
    {
      "@context": "https://schema.org",
      "@type": "Chapter",
      "@id": `${SITE.url}${canonicalPublicPath}#chapter`,
      url: `${SITE.url}${canonicalPublicPath}`,
      name: copy.seo.surahTitle(surah.num, surah.name),
      alternateName: surah.arabic,
      position: surah.num,
      inLanguage: contentLanguage,
      isPartOf: { "@id": `${SITE.url}/#quran` },
    },
    {
      "@context": "https://schema.org",
      "@type": "Book",
      "@id": `${SITE.url}/#quran`,
      url: `${SITE.url}/`,
      name: copy.seo.quranBook,
      inLanguage: "ar",
    },
  ]);

  function requestedAyah(): number | null {
    const hash = new RegExp(`^#ayah-${surah.num}-(\\d+)$`).exec(page.url.hash)?.[1];
    // ?v={surah}:{ayah} is the write-side of the scroll handler (share/reload parity);
    // ?verse= is the legacy spelling. A foreign surah number in ?v= is ignored.
    const shared = /^(\d+):(\d+)$/.exec(page.url.searchParams.get("v") ?? "");
    const legacy = page.url.searchParams.get("verse");
    let value: number | undefined;
    if (hash !== undefined) {
      value = Number(hash);
    } else if (shared && Number(shared[1]) === surah.num) {
      value = Number(shared[2]);
    } else if (legacy !== null) {
      value = Number(legacy);
    }
    return value !== undefined && Number.isSafeInteger(value) && value >= 1 && value <= surah.ayahCount
      ? value
      : null;
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
      // The target page streams in place through the reader's anchor-preserving
      // queue — no navigation, the path never moves.
      await readerView?.ensureAyah(targetPage.localPage, `${surah.num}:${ayah}`);
      const targetHref = publicHref(
        readerHrefFor(copy.locale, surahAyahPathFor(routeContext, surah, ayah)),
      );
      if (page.url.href !== new URL(targetHref, page.url.href).href) {
        replaceState(withModeParam(targetHref, reader.mode, page.url), page.state);
      }
      const row = await ayahRow(ayah);
      if (!row) return;
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
    void loadQuranData()
      .then(() => {
        const quranData = peekQuranData();
        if (quranData) reader.setPosition(positionForGlobal(quranData, activePage.page.startGlobal));
      })
      .catch(() => {});
  });

  // Sticky-bar position: server payload knows the initial page; the reader
  // republishes on scroll. Reading peekQuranData here keeps SSR free of the
  // client-only division fetch.
  $effect(() => {
    const startGlobal = activePage.page.startGlobal;
    const quranData = peekQuranData();
    if (quranData) reader.setPosition(positionForGlobal(quranData, startGlobal));
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
  path={canonicalPublicPath}
  title={seoTitle}
  description={seoDescription}
  extraLd={chapterLd}
  includeTextVariants
  includePlainVariant={false}
  inLanguage={contentLanguage}
  noindex={translationPending}
  crumbs={[
    { name: copy.seo.home, href: "/" },
    { name: copy.seo.breadcrumbSurah(surah.name), href: currentPublicPath },
  ]}
/>

<ReaderShell position={{ globalPage: activePage.page.globalPage, juz: data.juz, hizb: data.hizb }}>
  {#snippet header()}
    <span class="hidden min-w-0 truncate text-sm font-medium text-foreground-secondary sm:inline">
      {surah.num}. {surah.name}
      <span dir="rtl" lang="ar" class="ms-1 font-arabic text-base">{surah.arabic}</span>
    </span>
  {/snippet}

  {#if reader.hasQuery}
    <Results />
  {:else}
    <SurahReader
      bind:this={readerView}
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
