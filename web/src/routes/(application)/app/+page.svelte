<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { Button, Card, Seo } from "$lib/components";
  import ReaderPrerenderLinks from "$lib/components/i18n/ReaderPrerenderLinks.svelte";
  import { reader } from "$lib/stores/reader.svelte";
  import { readerSource } from "$lib/stores/reader-settings.svelte";
  import { prefs } from "$lib/stores/prefs.svelte";
  import type { QuranData } from "$lib/data/quran-data";
  import { loadQuranData, peekQuranData } from "$lib/data/quran-data-client";
  import { surahPathFor, surahRouteContext, type SurahRouteContext } from "$lib/data/quran";
  import { bakedTranslationCatalogue, findCatalogueEntry } from "$lib/quran/catalogue";
  import { getReaderUiCopy } from "$lib/i18n/reader-copy";
  import { readerHrefFor, readerHomeHrefFor } from "$lib/i18n/reader";
  import { publicHref } from "$lib/i18n/public-href";
  import { resumeToLastRead, resumeToVerse } from "$lib/reader/resume";

  let { data } = $props();
  const copy = getReaderUiCopy();

  let quranData = $state<QuranData | undefined>(peekQuranData());

  function sourceLabel(sourceId: string | undefined): string {
    if (!sourceId) return "";
    const entry = findCatalogueEntry(bakedTranslationCatalogue(), sourceId);
    return entry?.kind === "translation" ? entry.entry.name : "";
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
    reader.hydrate();
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
    {@const fraction = total > 0 ? Math.min(1, lr.n / total) : 0}
    {@const label = sourceLabel(lr.sourceId)}
    <Card class="max-w-xl">
      <p class="text-xs font-medium uppercase tracking-wide text-fg-3">Continue reading</p>
      <h2 class="mt-3 text-2xl font-semibold text-fg">
        {surah?.name ?? `Surah ${lr.num}`}
      </h2>
      <p class="mt-1 text-sm text-fg-2">
        {surah?.transliteration ?? ""}{surah?.meaning ? ` · ${surah.meaning}` : ""}
      </p>
      <p class="mt-3 text-sm text-fg-2">
        <span class="font-medium text-fg">{lr.num}:{lr.n}</span>{label ? ` · ${label}` : ""}
      </p>
      {#if total > 0}
        <div
          class="mt-4 h-1.5 w-full overflow-hidden rounded-full bg-bg-3"
          role="progressbar"
          aria-valuenow={lr.n}
          aria-valuemax={total}
        >
          <div class="h-full rounded-full bg-accent" style:width="{Math.round(fraction * 100)}%"></div>
        </div>
      {/if}
      <div class="mt-6 flex items-center gap-3">
        <Button onclick={() => resumeToLastRead({ kind: "arabic" })} arrow>Continue</Button>
        <label class="ml-auto flex cursor-pointer select-none items-center gap-2 text-xs text-fg-3">
          <input
            type="checkbox"
            class="h-4 w-4 accent-[var(--accent)]"
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
        <p class="text-xs font-medium uppercase tracking-wide text-fg-3">Recent</p>
        <div class="mt-1 flex flex-col divide-y divide-line rounded-lg border border-line bg-bg-1">
          {#each others as r (r.num + ":" + r.n + ":" + (r.sourceId ?? ""))}
            {@const rsurah = quranData?.surahByNum(r.num)}
            <button
              type="button"
              class="flex items-center gap-3 px-4 py-3 text-left transition-colors hover:bg-bg-2"
              onclick={() => resumeToVerse(r.num, r.n, r.sourceId, { kind: "arabic" })}
            >
              <span class="text-sm font-medium text-fg">{rsurah?.name ?? `Surah ${r.num}`}</span>
              <span class="text-sm text-fg-2">{r.num}:{r.n}</span>
              {#if sourceLabel(r.sourceId)}
                <span class="ml-auto text-xs text-fg-3">{sourceLabel(r.sourceId)}</span>
              {/if}
            </button>
          {/each}
        </div>
      </div>
    {/if}
  {:else}
    <Card class="max-w-xl">
      <h2 class="text-2xl font-semibold text-fg">Start reading</h2>
      <p class="mt-1 text-sm text-fg-2">
        Pick a place to begin. Your spot is saved automatically as you read.
      </p>
      <div class="mt-5 flex flex-wrap gap-2">
        <Button variant="ghost" onclick={() => openSurah(1)}>Al-Fātiḥah</Button>
        <Button variant="ghost" onclick={() => openSurah(2)}>Al-Baqarah</Button>
        <Button variant="ghost" onclick={() => openSurah(36)}>Yā-Sīn</Button>
        <Button variant="ghost" onclick={() => openSurah(67)}>Al-Mulk</Button>
      </div>
    </Card>
  {/if}
</div>
