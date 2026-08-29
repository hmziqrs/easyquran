<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { Button, Card, Icon, MetricCard, Panel, Seo } from "$lib/components";
  import ReaderPrerenderLinks from "$lib/components/i18n/ReaderPrerenderLinks.svelte";
  import { reader } from "$lib/stores/reader.svelte";
  import { readerSource } from "$lib/stores/reader-settings.svelte";
  import { prefs } from "$lib/stores/prefs.svelte";
  import { bookmarks } from "$lib/bookmarks/store.svelte";
  import type { QuranData } from "$lib/data/quran-data";
  import { RangeKind } from "$lib/data/quran-data";
  import { loadQuranData, peekQuranData } from "$lib/data/quran-data-client";
  import { surahPathFor, surahRouteContext, type SurahRouteContext } from "$lib/data/quran";
  import { peekTranslationName } from "$lib/quran/catalogue";
  import { getReaderUiCopy } from "$lib/i18n/reader-copy";
  import {
    landing_metric_bookmarks,
    landing_metric_bookmarks_empty,
    landing_metric_bookmarks_note,
    landing_metric_juz,
    landing_metric_juz_note,
    landing_metric_pages,
    landing_metric_pages_note,
    landing_metric_surahs,
    landing_metric_surahs_note,
    landing_metric_yours,
  } from "$lib/i18n/m/landing";
  import { readerHrefFor, readerHomeHrefFor } from "$lib/i18n/reader";
  import { publicHref } from "$lib/i18n/public-href";
  import { resumeToLastRead, resumeToVerse } from "$lib/reader/resume";
  import { parseModeParam } from "$lib/reader/mode-param";

  let { data } = $props();
  const copy = getReaderUiCopy();

  let quranData = $state<QuranData | undefined>(peekQuranData());

  // Metric strip copy reuses the landing namespace (same numerals, same labels) —
  // en+ar already shipped with the landing surface; no new copy.
  const msgLocale = { locale: copy.locale };
  const metrics = {
    surahs: landing_metric_surahs(undefined, msgLocale),
    surahsNote: landing_metric_surahs_note(undefined, msgLocale),
    juz: landing_metric_juz(undefined, msgLocale),
    juzNote: landing_metric_juz_note(undefined, msgLocale),
    pages: landing_metric_pages(undefined, msgLocale),
    pagesNote: landing_metric_pages_note(undefined, msgLocale),
    bookmarks: landing_metric_bookmarks(undefined, msgLocale),
    yours: landing_metric_yours(undefined, msgLocale),
    bookmarksEmpty: landing_metric_bookmarks_empty(undefined, msgLocale),
    bookmarksNote: landing_metric_bookmarks_note(undefined, msgLocale),
  };

  // Landing parity: constants mirror RANGE_COUNTS (same fallbacks as the marketing load).
  const FALLBACK_SURAH_COUNT = 114;
  const FALLBACK_JUZ_COUNT = 30;
  const FALLBACK_PAGE_COUNT = 604;
  const surahCount = $derived(quranData?.surahs.length ?? FALLBACK_SURAH_COUNT);
  const juzCount = $derived(quranData?.ranges(RangeKind.Juz).length ?? FALLBACK_JUZ_COUNT);
  const quranPageCount = $derived(quranData?.ranges(RangeKind.Page).length ?? FALLBACK_PAGE_COUNT);
  const bookmarkCount = $derived.by(() => {
    if (bookmarks.authed) return bookmarks.bookmarks.length;
    return reader.bookmarkedKeys.length;
  });

  function bookmarkValue(): string {
    if (bookmarkCount > 0) return String(bookmarkCount);
    return metrics.yours;
  }
  function bookmarkCaption(): string {
    if (bookmarkCount === 0) return metrics.bookmarksEmpty;
    return metrics.bookmarksNote;
  }

  function openSurah(num: number): void {
    reader.openVerse(num, 1);
    const surah = quranData?.surahByNum(num);
    if (!surah) return;
    const sourceId = readerSource.sourceId;
    const ctx: SurahRouteContext = sourceId ? surahRouteContext(sourceId) : { kind: "arabic" };
    void goto(publicHref(readerHrefFor(copy.locale, surahPathFor(ctx, surah))), {
      replaceState: true,
    });
  }

  onMount(() => {
    reader.hydrate(parseModeParam(page.url) ?? undefined);
    prefs.hydrate();
    if (prefs.instantResume && reader.hasLastRead) {
      void resumeToLastRead({ kind: "arabic" }, { replaceState: true });
      return;
    }
    if (!quranData) void loadQuranData().then((d) => (quranData = d));
  });
</script>

<Seo path={readerHomeHrefFor("en")} title={copy.seo.home} noindex />
<ReaderPrerenderLinks hrefs={data.readerPrerenderHrefs} />

<div class="mx-auto max-w-[1320px] px-5 pt-16 sm:px-7">
  {#if reader.hasLastRead}
    {@const lr = reader.lastRead!}
    {@const surah = quranData?.surahByNum(lr.num)}
    {@const total = surah?.ayahCount ?? 0}
    {@const fraction = reader.progressFor(lr.num) ?? (total > 0 ? Math.min(1, lr.n / total) : 0)}
    {@const label = peekTranslationName(lr.sourceId)}
    <Card class="max-w-xl">
      <p class="text-caption font-semibold uppercase tracking-wide text-muted">Continue reading</p>
      <h2 class="mt-3 text-2xl font-semibold text-foreground">
        {surah?.name ?? `Surah ${lr.num}`}
      </h2>
      <p class="mt-1 text-sm text-foreground-secondary">
        {surah?.transliteration ?? ""}{surah?.meaning ? ` · ${surah.meaning}` : ""}
      </p>
      <p class="mt-3 text-sm text-foreground-secondary">
        <span class="font-medium text-foreground">{lr.num}:{lr.n}</span>{label ? ` · ${label}` : ""}
      </p>
      {#if total > 0}
        <div
          class="mt-4 h-1.5 w-full overflow-hidden rounded-pill bg-surface-hover"
          role="progressbar"
          aria-valuenow={lr.n}
          aria-valuemax={total}
        >
          <div class="h-full rounded-pill bg-primary" style:width="{Math.round(fraction * 100)}%"></div>
        </div>
      {/if}
      <div class="mt-6 flex items-center gap-3">
        <Button onclick={() => resumeToLastRead({ kind: "arabic" })} arrow>Continue</Button>
        <label class="ms-auto flex cursor-pointer select-none items-center gap-2 text-xs text-muted">
          <input
            type="checkbox"
            class="h-4 w-4 accent-primary"
            checked={prefs.instantResume}
            onchange={(e) => prefs.setInstantResume(e.currentTarget.checked)}
          />
          Resume instantly
        </label>
      </div>
    </Card>
    {@const others = reader.recentReads.filter((r) => r.num !== lr.num || r.sourceId !== lr.sourceId)}
    {#if others.length > 0}
      <div class="mt-6 max-w-xl">
        <p class="text-xs font-medium uppercase tracking-wide text-muted">Recent</p>
        <div class="mt-1 flex flex-col divide-y divide-border rounded-lg border border-border bg-surface">
          {#each others as r (r.num + ":" + r.n + ":" + (r.sourceId ?? ""))}
            {@const rsurah = quranData?.surahByNum(r.num)}
            <button
              type="button"
              class="flex items-center gap-3 px-4 py-3 text-start transition-colors hover:bg-surface-hover"
              onclick={() => resumeToVerse(r.num, r.n, r.sourceId, { kind: "arabic" })}
            >
              <span class="text-sm font-medium text-foreground">{rsurah?.name ?? `Surah ${r.num}`}</span>
              <span class="text-sm text-foreground-secondary">{r.num}:{r.n}</span>
              {#if peekTranslationName(r.sourceId)}
                <span class="ms-auto text-xs text-muted">{peekTranslationName(r.sourceId)}</span>
              {/if}
            </button>
          {/each}
        </div>
      </div>
    {/if}
  {:else}
    <!-- Landing grammar carried into the app: accent hero band (white pill CTAs on
         the primary fill) + the gapless four-hue metric strip, same MetricCards the
         marketing home renders. -->
    <Panel variant="accent" class="max-w-5xl px-7 py-8 sm:px-9 sm:py-10">
      <!-- Global h1–h4 rule pins --foreground; on the accent fill the heading must
           opt back into the on-primary pair explicitly. -->
      <h2 class="text-2xl font-semibold text-primary-foreground">Start reading</h2>
      <p class="mt-1 text-sm opacity-85">Pick a place to begin. Your spot is saved automatically as you read.</p>
      <div class="mt-5 flex flex-wrap gap-2">
        <button
          type="button"
          onclick={() => openSurah(1)}
          class="rounded-pill bg-primary-foreground px-5 py-2.5 text-[14px] font-bold text-primary transition-[filter] duration-150 ease-out hover:brightness-[0.96] focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary-foreground"
        >
          Al-Fātiḥah
        </button>
        <button
          type="button"
          onclick={() => openSurah(2)}
          class="rounded-pill px-5 py-2.5 text-[14px] font-bold text-primary-foreground transition-colors duration-150 ease-out hover:bg-primary-foreground/15 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary-foreground"
        >
          Al-Baqarah
        </button>
        <button
          type="button"
          onclick={() => openSurah(36)}
          class="rounded-pill px-5 py-2.5 text-[14px] font-bold text-primary-foreground transition-colors duration-150 ease-out hover:bg-primary-foreground/15 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary-foreground"
        >
          Yā-Sīn
        </button>
        <button
          type="button"
          onclick={() => openSurah(67)}
          class="rounded-pill px-5 py-2.5 text-[14px] font-bold text-primary-foreground transition-colors duration-150 ease-out hover:bg-primary-foreground/15 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary-foreground"
        >
          Al-Mulk
        </button>
      </div>
    </Panel>
    <div class="mt-4 max-w-5xl overflow-hidden rounded-xl border border-border">
      <div class="grid grid-cols-1 gap-0 sm:grid-cols-2 lg:grid-cols-4">
        <MetricCard hue={1} value={String(surahCount)} label={metrics.surahs} caption={metrics.surahsNote}>
          <Icon name="book" />
        </MetricCard>
        <MetricCard hue={2} value={String(juzCount)} label={metrics.juz} caption={metrics.juzNote}>
          <Icon name="continuous" />
        </MetricCard>
        <MetricCard hue={3} value={String(quranPageCount)} label={metrics.pages} caption={metrics.pagesNote}>
          <Icon name="note" />
        </MetricCard>
        <MetricCard hue={4} value={bookmarkValue()} label={metrics.bookmarks} caption={bookmarkCaption()}>
          <Icon name="bookmark" />
        </MetricCard>
      </div>
    </div>
  {/if}
</div>
