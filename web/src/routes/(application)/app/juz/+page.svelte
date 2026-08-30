<script lang="ts">
  import { Seo, Icon } from "$lib/components";
  import { globalPagePathFor, juzPathFor, type SurahRouteContext } from "$lib/data/quran";
  import { getReaderUiCopy } from "$lib/i18n/reader-copy";
  import { readerHrefFor } from "$lib/i18n/reader";
  import { readerCanonicalEntryPath } from "$lib/i18n/seo";
  import { publicHref } from "$lib/i18n/public-href";
  import ReaderShell from "../_reader/ReaderShell.svelte";
  import { HUE_LEGIBLE, HUE_SOFT, hueSlotFor } from "../_reader/hue-slot";
  let { data } = $props();

  const arabicCtx: SurahRouteContext = { kind: "arabic" };
  const copy = getReaderUiCopy();

  const seoTitle = $derived(copy.seo.juzIndexTitle);
  const seoDescription = $derived(copy.seo.juzIndexDescription);

  function juzHref(n: number): `/${string}` {
    return readerHrefFor(copy.locale, juzPathFor(arabicCtx, n));
  }

  function pageHref(n: number): `/${string}` {
    return readerHrefFor(copy.locale, globalPagePathFor(arabicCtx, n));
  }

  function sajdaLabel(count: number): string {
    if (count === 1) return copy.index.sajda;
    return copy.index.sajdaCount(count);
  }

  function sajdaTitle(refs: readonly { surah: number; ayah: number }[]): string {
    return refs.map((ref) => `${ref.surah}:${ref.ayah}`).join(", ");
  }
</script>

<Seo
  path={readerCanonicalEntryPath("juz-index")}
  title={seoTitle}
  description={seoDescription}
  includeTextVariants={false}
/>

<ReaderShell>
  {#snippet header()}
    <h1 class="text-sm font-medium text-foreground-secondary">{copy.index.juzTitle}</h1>
    <span class="ms-auto font-mono text-[12px] text-muted"
      >{copy.range.juzCount(data.ajzur.length)}</span
    >
  {/snippet}

  <ul class="grid grid-cols-1 gap-3 lg:grid-cols-2">
    {#each data.ajzur as juz (juz.index)}
      {@const hue = hueSlotFor(juz.index)}
      <li class="overflow-hidden rounded-lg border border-border">
        <div class="flex items-center gap-3 px-4 py-3">
          <a
            href={publicHref(juzHref(juz.index))}
            data-sveltekit-preload-data="hover"
            aria-label={copy.range.item("juz", juz.index)}
            class="flex min-w-0 flex-1 items-center gap-3 transition-colors hover:bg-surface-hover focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
          >
            <span
              class="flex h-9 min-w-11 flex-none items-center justify-center rounded-pill px-2.5 text-[15px] font-extrabold tabular-nums"
              style:background={HUE_SOFT[hue]}
              style:color={HUE_LEGIBLE[hue]}
            >
              {juz.index}
            </span>
            <span class="flex min-w-0 flex-1 flex-col gap-0.5">
              <span class="text-sm font-medium text-foreground"
                >{copy.range.item("juz", juz.index)}</span
              >
              <span class="font-mono text-[11px] text-muted">{juz.first} – {juz.last}</span>
            </span>
          </a>
          {#if juz.sajdas.length > 0}
            <span
              class="flex flex-none items-center gap-1.5 rounded-pill px-2.5 py-1 text-[11.5px] font-medium"
              style:background={HUE_SOFT[hue]}
              style:color={HUE_LEGIBLE[hue]}
              title={sajdaTitle(juz.sajdas)}
            >
              <Icon name="moon" size={12} />
              {sajdaLabel(juz.sajdas.length)}
            </span>
          {/if}
        </div>
        <div class="grid grid-cols-2 gap-px border-t border-border bg-border lg:grid-cols-4">
          {#each juz.quarters as quarter, qi (quarter.first)}
            <a
              href={publicHref(pageHref(quarter.page))}
              data-sveltekit-preload-data="hover"
              class="flex flex-col gap-0.5 bg-surface px-3 py-2 transition-colors hover:bg-surface-hover focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
            >
              <span class="text-[11.5px] font-semibold text-foreground-secondary"
                >{copy.index.quarter(qi + 1)}</span
              >
              <span class="truncate font-mono text-[10.5px] text-muted"
                >{quarter.first} – {quarter.last}</span
              >
            </a>
          {/each}
        </div>
      </li>
    {/each}
  </ul>
</ReaderShell>
