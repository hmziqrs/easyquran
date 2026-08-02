<script lang="ts">
  import { untrack } from "svelte";
  import { page } from "$app/state";
  import { Seo } from "$lib/components";
  import { reader } from "$lib/stores/reader.svelte";
  import ReaderShell from "../_reader/ReaderShell.svelte";
  import SurahReader from "../_reader/SurahReader.svelte";
  import Results from "../_reader/Results.svelte";

  let { data } = $props();
  const surah = $derived(data.surah);
  const slug = $derived(page.params.surah as string);

  $effect(() => {
    const num = surah.num;
    untrack(() => {
      reader.setCurrent(num);
      reader.seedSurah(num, surah.verses);
      void reader.refreshFromWorker(num);
    });
  });

  $effect(() => {
    const v = page.url.searchParams.get("verse");
    if (!v) return;
    document
      .getElementById(`ayah-${v}`)
      ?.scrollIntoView({ behavior: "smooth", block: "center" });
  });

  const seoTitle = $derived(`Surah ${surah.num}, ${surah.name} — Arabic Text & Reading · EasyQuran`);
  const seoDescription = $derived(
    `Read Surah ${surah.name} (${surah.arabic}) — ${surah.ayahCount} ayahs, ${surah.place === "meccan" ? "Meccan" : "Medinan"}, in the Uthmani script. Free, fast, and works offline.`,
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

<ReaderShell>
  {#snippet header()}
    <span class="text-sm font-medium text-fg-2">{surah.num}. {surah.name}</span>
    <span dir="rtl" class="ml-auto font-arabic text-base text-fg-3">{surah.arabic}</span>
  {/snippet}

  {#if reader.hasQuery}
    <Results />
  {:else}
    <SurahReader {surah} previous={data.previous} next={data.next} />
  {/if}
</ReaderShell>
