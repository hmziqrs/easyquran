<script lang="ts">
  import { onMount } from "svelte";
  import { readingAnalytics, type ReadingSummary } from "#lib/analytics/reading-analytics.js";
  import type { QuranData } from "#lib/data/quran-data.js";
  import type { UiLocale } from "#lib/i18n/locales.js";
  import {
    reader_insights_summary,
    reader_insights_top_ayahs,
    reader_insights_top_surahs,
    reader_widget_most_read_empty,
  } from "#lib/i18n/m/reader.js";

  // Personal reading insights: totals, most-read surahs and verses. Server
  // summary when signed in, the device rollup otherwise.
  let { quranData, locale }: { quranData: QuranData | undefined; locale: UiLocale } = $props();

  const msg = $derived({ locale });
  let summary = $state.raw<ReadingSummary | null>(null);
  let loaded = $state(false);

  const empty = $derived(summary === null || summary.totals.ayahs === 0);

  function surahName(num: number): string {
    return quranData?.surahByNum(num)?.name ?? `Surah ${num}`;
  }

  onMount(() => {
    void readingAnalytics.fetchSummary().then((result) => {
      summary = result;
      loaded = true;
    });
  });
</script>

{#if !loaded}
  <p class="mt-2 text-[13px] text-muted" aria-busy="true">…</p>
{:else if empty || summary === null}
  <p class="mt-2 text-[13px] text-muted">{reader_widget_most_read_empty(undefined, msg)}</p>
{:else}
  <div class="mt-2 overflow-hidden rounded-md border border-border bg-surface">
    <p class="border-b border-border px-4 py-3 text-sm text-foreground">
      {reader_insights_summary(
        {
          ayahs: summary.totals.ayahs,
          minutes: Math.round(summary.totals.ms / 60_000),
          days: summary.totals.days,
        },
        msg,
      )}
    </p>
    <div class="grid gap-0 sm:grid-cols-2 sm:divide-x sm:divide-border">
      <section class="px-4 py-3">
        <h3 class="eyebrow">{reader_insights_top_surahs(undefined, msg)}</h3>
        <ol class="mt-2 flex flex-col gap-1.5">
          {#each summary.topSurahs as entry (entry.surah)}
            <li class="flex items-center gap-3 text-sm">
              <span class="min-w-0 flex-1 truncate text-foreground">{surahName(entry.surah)}</span>
              <span class="flex-none tabular-nums text-muted">{entry.reads}</span>
            </li>
          {/each}
        </ol>
      </section>
      <section class="border-t border-border px-4 py-3 sm:border-t-0">
        <h3 class="eyebrow">{reader_insights_top_ayahs(undefined, msg)}</h3>
        <ol class="mt-2 flex flex-col gap-1.5">
          {#each summary.topAyahs as entry (`${entry.surah}:${entry.ayah}`)}
            <li class="flex items-center gap-3 text-sm">
              <span class="min-w-0 flex-1 truncate text-foreground">
                {surahName(entry.surah)} <span class="text-muted">{entry.surah}:{entry.ayah}</span>
              </span>
              <span class="flex-none tabular-nums text-muted">{entry.reads}</span>
            </li>
          {/each}
        </ol>
      </section>
    </div>
  </div>
{/if}
