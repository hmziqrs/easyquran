<script lang="ts">
  import { Seo, Icon } from "#lib/components/index.js";
  import { hizbPathFor, juzPathFor, resumeCtxFor } from "#lib/data/quran.js";
  import { getReaderUiCopy } from "#lib/i18n/reader-copy.js";
  import { readerHrefFor } from "#lib/i18n/reader.js";
  import { readerCanonicalEntryPath } from "#lib/i18n/seo.js";
  import { publicHref } from "#lib/i18n/public-href.js";
  import ReaderShell from "../_reader/ReaderShell.svelte";
  import { reader } from "#lib/stores/reader.svelte.js";
  import { HUE_LEGIBLE, HUE_SOFT, hueSlotFor } from "../_reader/hue-slot";
  import type { JuzBound, JuzSajdaRef } from "./+page";
  let { data } = $props();

  // Rows carry the reader's active source after hydration; prerendered HTML stays Arabic.
  const ctx = $derived(resumeCtxFor(reader.lastRead, { kind: "arabic" }));
  const copy = getReaderUiCopy();

  const seoTitle = $derived(copy.seo.juzIndexTitle);
  const seoDescription = $derived(copy.seo.juzIndexDescription);

  function juzHref(n: number): `/${string}` {
    return readerHrefFor(copy.locale, juzPathFor(ctx, n));
  }

  function hizbHref(n: number): `/${string}` {
    return readerHrefFor(copy.locale, hizbPathFor(ctx, n));
  }

  /** "Al-Fatihah 1:1" — the surah name reads faster than a bare verse key. */
  function boundLabel(bound: JuzBound): string {
    return `${bound.surahName} ${bound.key}`;
  }

  function sajdaLabel(count: number): string {
    if (count === 1) return copy.index.sajda;
    return copy.index.sajdaCount(count);
  }

  function sajdaTitle(refs: readonly JuzSajdaRef[]): string {
    return refs.map((ref) => `${ref.surah}:${ref.ayah}`).join(", ");
  }
</script>

<Seo
  path={readerCanonicalEntryPath("juz-index")}
  title={seoTitle}
  description={seoDescription}
  includeTextVariants={false}
/>

<ReaderShell textTools={false}>
  {#snippet header()}
    <h1 class="text-sm font-medium text-foreground-secondary">{copy.index.juzTitle}</h1>
    <span class="ms-auto text-[12.5px] text-muted"
      >{copy.range.juzCount(data.ajzur.length)}</span
    >
  {/snippet}

  <!-- One card per juz: the juz itself (number, where it starts and ends, sajdas), then its
       two hizbs — the half-juz units with their own reader routes. -->
  <ul class="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
    {#each data.ajzur as juz (juz.index)}
      {@const hue = hueSlotFor(juz.index)}
      <li
        class="flex flex-col overflow-hidden rounded-lg border border-border transition-colors hover:border-border-strong"
      >
        <a
          href={publicHref(juzHref(juz.index))}
          data-sveltekit-preload-data="hover"
          class="flex min-w-0 flex-1 items-center gap-3.5 px-4 py-3.5 transition-colors hover:bg-surface-hover focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-focus-ring"
        >
          <span
            class="flex h-9 min-w-11 flex-none items-center justify-center rounded-pill px-2.5 text-[15px] font-medium"
            style:background={HUE_SOFT[hue]}
            style:color={HUE_LEGIBLE[hue]}
          >
            {juz.index}
          </span>
          <span class="flex min-w-0 flex-1 flex-col gap-0.5">
            <span class="text-[15px] font-medium text-foreground"
              >{copy.range.item("juz", juz.index)}</span
            >
            <span class="truncate text-[12.5px] text-muted">
              {boundLabel(juz.first)} – {boundLabel(juz.last)}
            </span>
          </span>
          {#if juz.sajdas.length > 0}
            <span
              class="flex flex-none items-center gap-1 rounded-pill bg-background-subtle px-2 py-1 text-[11px] font-medium text-foreground-secondary"
              title={sajdaTitle(juz.sajdas)}
            >
              <Icon name="moon" size={11} />
              {sajdaLabel(juz.sajdas.length)}
            </span>
          {/if}
        </a>
        <div class="grid grid-cols-2 border-t border-border">
          {#each juz.hizbs as hizb, hi (hizb.index)}
            <a
              href={publicHref(hizbHref(hizb.index))}
              data-sveltekit-preload-data="hover"
              class={[
                "flex min-h-11 items-center justify-between gap-2 px-4 py-2 transition-colors hover:bg-surface-hover focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-focus-ring",
                hi > 0 && "border-s border-border",
              ]}
            >
              <span class="text-[12.5px] font-medium text-foreground-secondary"
                >{copy.range.item("hizb", hizb.index)}</span
              >
              <span class="text-[12px] text-muted">{hizb.first}</span>
            </a>
          {/each}
        </div>
      </li>
    {/each}
  </ul>
</ReaderShell>
