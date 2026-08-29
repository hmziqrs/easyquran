<script lang="ts">
  import { Seo, Chip } from "$lib/components";
  import { juzPathFor, type SurahRouteContext } from "$lib/data/quran";
  import { getReaderUiCopy } from "$lib/i18n/reader-copy";
  import { readerHrefFor } from "$lib/i18n/reader";
  import { readerCanonicalEntryPath } from "$lib/i18n/seo";
  import { publicHref } from "$lib/i18n/public-href";
  import ReaderShell from "../_reader/ReaderShell.svelte";

  // Landing surah-tile grammar (boards cycle the four hue slots by position):
  // soft fill + legible numeral, per §6 — the same pair the marketing index rows
  // render, so the juz list reads as the same product.
  const HUE_SOFT = {
    1: "var(--hue-1-soft)",
    2: "var(--hue-2-soft)",
    3: "var(--hue-3-soft)",
    4: "var(--hue-4-soft)",
  } as const;
  const HUE_LEGIBLE = {
    1: "var(--hue-1-legible)",
    2: "var(--hue-2-legible)",
    3: "var(--hue-3-legible)",
    4: "var(--hue-4-legible)",
  } as const;
  type Hue = keyof typeof HUE_SOFT;

  function hueFor(index: number): Hue {
    // SAFETY: ((index-1) % 4) is 0–3 for any positive integer, so +1 is exactly 1–4.
    return (((index - 1) % 4) + 1) as Hue;
  }

  let { data } = $props();

  const arabicCtx: SurahRouteContext = { kind: "arabic" };
  const copy = getReaderUiCopy();

  const seoTitle = $derived(copy.seo.juzIndexTitle);
  const seoDescription = $derived(copy.seo.juzIndexDescription);
</script>

<Seo
  path={readerCanonicalEntryPath("juz-index")}
  title={seoTitle}
  description={seoDescription}
  includeTextVariants={false}
/>

<ReaderShell>
  {#snippet header()}
    <h1 class="text-sm font-medium text-foreground-secondary">{copy.seo.juzIndexTitle}</h1>
    <span class="ms-auto font-mono text-[12px] text-muted"
      >{copy.range.juzCount(data.ajzur.length)}</span
    >
  {/snippet}

  <ul class="grid grid-cols-1 gap-2 sm:grid-cols-2">
    {#each data.ajzur as juz (juz.index)}
      {@const hue = hueFor(juz.index)}
      <li>
        <a
          href={publicHref(readerHrefFor(copy.locale, juzPathFor(arabicCtx, juz.index)))}
          data-sveltekit-preload-data="hover"
          class="flex items-center gap-3 rounded-lg border border-border px-4 py-3 transition-colors hover:bg-surface-hover"
        >
          <span
            class="flex h-9 min-w-11 flex-none items-center justify-center rounded-pill px-2.5 text-[15px] font-extrabold tabular-nums"
            style:background={HUE_SOFT[hue]}
            style:color={HUE_LEGIBLE[hue]}
          >
            {juz.index}
          </span>
          <span class="text-sm font-medium text-foreground">{copy.range.item("juz", juz.index)}</span>
          <Chip accent class="ms-auto font-mono">{juz.first} – {juz.last}</Chip>
        </a>
      </li>
    {/each}
  </ul>
</ReaderShell>
