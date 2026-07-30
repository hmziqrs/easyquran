<!--
  /app/[surah] — the reader. The surah is derived from the URL param (single
  source of truth for what renders), so a deep link like /app/al-baqarah
  prerenders the right surah with no hydration mismatch.

  Layout mirrors quran.com/al-baqarah: the reading content is a centered column
  that owns the full width. The navigation sidebar is the shadcn-svelte Sidebar
  — a FIXED, off-canvas, animated panel that takes NO layout space while
  collapsed (unlike the old inline grid column); a SidebarTrigger toggles it
  open. The sticky Player mounts while audio plays.
-->
<script lang="ts">
  import { untrack } from "svelte";
  import { page } from "$app/state";
  import { Seo } from "$lib/components";
  import { reader } from "$lib/stores/reader.svelte";
  import { SidebarProvider, SidebarInset, SidebarTrigger } from "$lib/components/ui/sidebar";
  import AppSidebar from "../_reader/Sidebar.svelte";
  import SurahReader from "../_reader/SurahReader.svelte";
  import Results from "../_reader/Results.svelte";
  import Player from "../_reader/Player.svelte";

  // Verses arrive from the SSG server load (prerendered Uthmani text for SEO +
  // first paint, no backend needed). The URL param stays the source of truth for
  // which surah renders, so deep links like /app/al-baqarah prerender correctly
  // with no hydration mismatch.
  let { data } = $props();
  const surah = $derived(data.surah);
  const slug = $derived(page.params.surah as string);

  // Keep the store's "current" + the synchronous verse cache in sync with the
  // rendered surah. `untrack` is essential: setCurrent()/seedSurah() touch the
  // $state proxy; without untrack this effect would depend on notes/bookmarks/
  // mode/font and re-run on every keystroke in a note. untrack keeps the surah
  // as the sole dependency.
  $effect(() => {
    const num = surah.num;
    untrack(() => {
      reader.setCurrent(num);
      reader.seedSurah(num, surah.verses);
    });
  });

  // Deep link to a verse (?verse=N): scroll it into view on load + navigation.
  // Works in both modes — the ayah-{n} anchor exists in verse rows (VerseRow)
  // and in reading-mode markers (SurahReader).
  $effect(() => {
    const v = page.url.searchParams.get("verse");
    if (!v) return;
    document
      .getElementById(`ayah-${v}`)
      ?.scrollIntoView({ behavior: "smooth", block: "center" });
  });

  // Per-surah SEO (doc §5): indexable, with a canonical, description, and a
  // Chapter structured-data node. No .md/.txt variants for app routes.
  const seoTitle = $derived(`Surah ${surah.num}, ${surah.name} — Arabic Text & Reading · EasyQuran`);
  const seoDescription = $derived(
    `Read Surah ${surah.name} (${surah.arabic}) — ${surah.ayahCount} verses, ${surah.place === "meccan" ? "Meccan" : "Medinan"}, in the Uthmani script. Free, fast, and works offline.`,
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
</script>

<Seo
  path={`/app/${slug}`}
  title={seoTitle}
  description={seoDescription}
  extraLd={chapterLd}
  includeTextVariants={false}
/>

<SidebarProvider open={false}>
  <AppSidebar />

  <SidebarInset>
    <!-- top bar: sidebar toggle + current surah. Sticky beneath the global nav. -->
    <header
      class="sticky top-[60px] z-10 flex items-center gap-3 border-b border-line bg-bg/80 px-5 py-2.5 backdrop-blur-xl sm:px-7"
    >
      <SidebarTrigger />
      <span class="text-sm font-medium text-fg-2">{surah.num}. {surah.name}</span>
      <span dir="rtl" class="ml-auto font-arabic text-base text-fg-3">{surah.arabic}</span>
    </header>

    <!-- centered reading column — owns the full width -->
    <div class="mx-auto w-full max-w-[860px] px-5 py-6 sm:px-7">
      {#if reader.hasQuery}
        <Results />
      {:else}
        <SurahReader {surah} />
      {/if}
    </div>
  </SidebarInset>
</SidebarProvider>

<Player />
