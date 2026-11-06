<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/state";
  import { Container, Seo } from "#lib/components/index.js";
  import { resumeCtxFor, surahAyahPathFor, type SurahRouteContext } from "#lib/data/quran.js";
  import { marketingLocaleFromPath } from "#lib/i18n/marketing-copy.js";
  import {
    daily_ayah_date,
    daily_ayah_description,
    daily_ayah_heading,
    daily_ayah_intro,
    daily_ayah_open,
    daily_ayah_title,
    daily_ayah_unavailable,
  } from "#lib/i18n/m/daily.js";
  import { publicHref } from "#lib/i18n/public-href.js";
  import { readerHrefFor } from "#lib/i18n/reader.js";
  import { reader } from "#lib/stores/reader.svelte.js";

  let { data } = $props();

  const locale = $derived(marketingLocaleFromPath(page.url.pathname));
  const options = $derived({ locale });
  const ARABIC: SurahRouteContext = { kind: "arabic" };

  onMount(() => {
    // Idempotent; the palette also hydrates on marketing pages.
    reader.hydrate();
  });

  // The server cannot see the reader's last source, so it links to the Arabic
  // text. Once the reader has hydrated, a visitor who last read a translation
  // keeps that translation in the link.
  const readCtx = $derived(resumeCtxFor(reader.lastRead, ARABIC));
  const readHref = $derived.by(() => {
    const daily = data.daily;
    if (!daily) return null;
    const surah = { slug: daily.slug, num: daily.surah };
    return publicHref(readerHrefFor(locale, surahAyahPathFor(readCtx, surah, daily.ayah)));
  });
</script>

<Seo
  path="/daily-ayah"
  schemaSubtype="WebPage"
  title={daily_ayah_title(undefined, options)}
  description={daily_ayah_description(undefined, options)}
  inLanguage={locale}
  includeTextVariants={false}
  includePlainVariant={false}
/>

<Container class="py-12 md:py-16">
  <div class="flex max-w-[680px] flex-col gap-8">
    <div class="flex flex-col gap-3">
      <h1 class="text-h1">{daily_ayah_heading(undefined, options)}</h1>
      <p class="text-body-l text-foreground-secondary">{daily_ayah_intro(undefined, options)}</p>
    </div>

    {#if data.daily}
      <figure class="flex flex-col gap-5 rounded-lg border border-border bg-background-subtle p-6 md:p-8">
        <p dir="rtl" lang="ar" class="font-quran text-[26px] leading-[2] text-foreground md:text-[30px]">
          {data.daily.text}
        </p>
        <figcaption
          class="flex flex-wrap items-center justify-between gap-3 text-body text-foreground-secondary"
        >
          <span>{data.daily.name} {data.daily.surah}:{data.daily.ayah}</span>
          <span>{daily_ayah_date({ date: data.date }, options)}</span>
        </figcaption>
      </figure>
      {#if readHref}
        <a
          href={readHref}
          class="self-start text-foreground underline underline-offset-2 transition-colors hover:text-primary"
        >
          {daily_ayah_open(undefined, options)}
        </a>
      {/if}
    {:else}
      <p role="status" class="text-body text-foreground-secondary">
        {daily_ayah_unavailable(undefined, options)}
      </p>
    {/if}
  </div>
</Container>
