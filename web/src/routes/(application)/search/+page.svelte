<script lang="ts">
  import { onMount } from "svelte";
  import { replaceState } from "$app/navigation";
  import { page } from "$app/state";
  import { loadQuranData } from "#lib/data/quran-data-client.js";
  import { visibleUrl } from "#lib/reader/visible-url.js";
  import type { QuranData } from "#lib/data/quran-data.js";
  import { Icon } from "#lib/components/icon/index.js";
  import { TRANSLATION_CATALOGUE_BY_ID } from "#lib/quran/catalogue.js";
  import { MIN_QUERY_LEN } from "#lib/quran/search/normalize.js";
  import { getSearchCopy } from "#lib/i18n/search-copy.js";
  import { createSearchEngine } from "#lib/search/page/engine.svelte.js";
  import { parseSearchUrl, serializeSearchState } from "#lib/search/page/query-url.js";
  import { resolveDefaultSelection } from "#lib/search/page/selection.js";
  import type { SectionState } from "#lib/search/page/types.js";
  import { reader } from "#lib/stores/reader.svelte.js";
  import { searchRecents } from "#lib/stores/search-recents.svelte.js";
  import { searchSelection } from "#lib/stores/search-selection.svelte.js";
  import { stackedTranslations } from "#lib/stores/stacked-translations.svelte.js";
  import { storageReport } from "#lib/stores/storage-report.svelte.js";

  import NavSuggestions from "./_components/NavSuggestions.svelte";
  import ResultSection from "./_components/ResultSection.svelte";
  import SearchControls from "./_components/SearchControls.svelte";
  import SurahSuggestions from "./_components/SurahSuggestions.svelte";
  import TranslationPicker from "./_components/TranslationPicker.svelte";

  const copy = getSearchCopy();
  const engine = createSearchEngine();

  let quranData = $state<QuranData | null>(null);
  let pickerOpen = $state(false);
  let adoptedSelection = $state(false);
  let lastSelectionKey: string | null = null;

  const committedTooShort = $derived(
    engine.committedQuery.length < MIN_QUERY_LEN && engine.navSuggestions.length === 0,
  );

  const orderedSections = $derived.by(() => {
    void engine.sectionList;
    const byId = new Map(engine.sectionList.map((section) => [section.id, section]));
    const out: SectionState[] = [];
    const arabic = byId.get("arabic");
    if (arabic) out.push(arabic);
    for (const id of searchSelection.ids) {
      const section = byId.get(id);
      if (section && section.kind === "translation") out.push(section);
    }
    return out;
  });

  const selectedChips = $derived(
    searchSelection.ids
      .map((id) => TRANSLATION_CATALOGUE_BY_ID.get(id))
      .filter((entry): entry is NonNullable<typeof entry> => entry !== undefined)
      .map((entry) => ({ id: entry.id, name: entry.translator ?? entry.name })),
  );

  const suggestSourceId = $derived(reader.lastRead?.sourceId ?? "arabic");

  const showRecents = $derived(engine.inputQuery.length === 0 && searchRecents.queries.length > 0);

  onMount(() => {
    const initial = parseSearchUrl(page.url);
    searchSelection.setIds(
      resolveDefaultSelection({
        urlT: initial.t,
        persisted: searchSelection.ids,
        lastReadSourceId: reader.lastRead?.sourceId,
        stackedIds: stackedTranslations.ids,
      }),
    );
    adoptedSelection = true;
    if (initial.q.length > 0) engine.run(initial.q);
    void loadQuranData().then(
      (data) => {
        quranData = data;
      },
      () => undefined,
    );
    storageReport.hydrate();
    return () => engine.dispose();
  });

  $effect(() => {
    if (!adoptedSelection) return;
    const ids = searchSelection.ids;
    const key = ids.join(",");
    if (lastSelectionKey === null) {
      lastSelectionKey = key;
      return;
    }
    if (key === lastSelectionKey) return;
    lastSelectionKey = key;
    if (engine.committedQuery.length >= MIN_QUERY_LEN) engine.run(engine.committedQuery);
  });

  $effect(() => {
    void storageReport.artifacts;
    engine.onArtifactsChanged();
  });

  $effect(() => {
    if (!adoptedSelection || engine.inputQuery.trim() !== engine.committedQuery) return;
    // kit 3: shallow writes land in page.shallow.url, never page.url — compare
    // against the visible URL or this effect loops on its own write forever.
    const url = visibleUrl(page);
    const next = serializeSearchState(engine.committedQuery, searchSelection.ids);
    if (next === url.search) return;
    try {
      replaceState(`${url.pathname}${next}`, page.state);
    } catch {
      return;
    }
  });

  function togglePicker(): void {
    pickerOpen = !pickerOpen;
  }
</script>

<svelte:head>
  <title>{copy.title} · EasyQuran</title>
</svelte:head>

<div lang={copy.locale} dir={copy.direction}>
  <div class="mx-auto max-w-[1180px] px-6 pt-5 pb-10 sm:px-7 sm:pt-6 sm:pb-12">
    <h1 class="text-[28px] font-medium leading-tight tracking-[-0.02em] text-foreground">{copy.title}</h1>

    <div class="mt-5 flex flex-col gap-4 sm:mt-6">
      <SearchControls
        {copy}
        query={engine.inputQuery}
        searching={engine.searching}
        selected={selectedChips}
        {pickerOpen}
        onQuery={(value) => engine.run(value)}
        onRemove={(id) => searchSelection.remove(id)}
        onTogglePicker={togglePicker}
      />

      {#if pickerOpen}
        <TranslationPicker {copy} />
      {/if}

      {#if showRecents}
        <!-- Local-only history: pause stops recording, clear wipes the device list; never synced (docs/search-system.md §5.1). -->
        <section aria-label={copy.recentsHeading} class="flex flex-col gap-2">
          <div class="flex items-center justify-between gap-3">
            <h2 class="px-1 text-[12px] font-medium text-muted">{copy.recentsHeading}</h2>
            <div class="flex items-center gap-3">
              <button
                type="button"
                onclick={() => searchRecents.setPaused(!searchRecents.paused)}
                class="text-[12.5px] text-muted underline-offset-4 transition-colors hover:text-foreground hover:underline"
              >
                {searchRecents.paused ? copy.recentsResume : copy.recentsPause}
              </button>
              <button
                type="button"
                onclick={() => searchRecents.clear()}
                class="text-[12.5px] text-muted underline-offset-4 transition-colors hover:text-foreground hover:underline"
              >
                {copy.recentsClear}
              </button>
            </div>
          </div>
          <div class="flex flex-wrap gap-2">
            {#each searchRecents.queries as q (q)}
              <button
                type="button"
                onclick={() => engine.run(q)}
                class="rounded-pill border border-border px-3 py-1.5 text-[12.5px] text-foreground-secondary transition-colors hover:border-border-strong hover:text-foreground"
              >
                {q}
              </button>
            {/each}
          </div>
        </section>
      {/if}

      <NavSuggestions
        {copy}
        matches={engine.navSuggestions}
        {quranData}
        sourceId={suggestSourceId}
      />

      <SurahSuggestions
        {copy}
        suggestions={engine.surahSuggestions}
        {quranData}
        sourceId={suggestSourceId}
      />

      <div class="mt-2 flex flex-col gap-6" aria-label={copy.resultsLabel}>
        {#if committedTooShort}
          <!-- Empty state carries the hue-2 soft/legible pair (§6) so the page is not
               grey-on-grey before the first query. -->
          <div class="flex items-center gap-3.5">
            <span
              class="flex size-11 flex-none items-center justify-center rounded-sm text-[var(--hue-2-legible)]"
              style:background="var(--hue-2-soft)"
              aria-hidden="true"
            >
              <Icon name="search" size={20} />
            </span>
            {#if engine.inputQuery.length === 0}
              <p class="text-caption text-muted">{copy.emptyIdle}</p>
            {:else}
              <p class="text-caption text-muted">{copy.tooShort}</p>
            {/if}
          </div>
        {:else if orderedSections.length === 0}
          <div class="flex items-center gap-3.5">
            <span
              class="flex size-11 flex-none items-center justify-center rounded-sm text-[var(--hue-2-legible)]"
              style:background="var(--hue-2-soft)"
              aria-hidden="true"
            >
              <Icon name="search" size={20} />
            </span>
            <p class="text-caption text-muted">{copy.pickPrompt}</p>
          </div>
        {:else}
          {#each orderedSections as section (section.id)}
            <ResultSection
              {section}
              {copy}
              {quranData}
              onRetry={(sectionId) => engine.retry(sectionId)}
              onLoadMore={(sectionId) => engine.loadMore(sectionId)}
            />
          {/each}
        {/if}
      </div>
    </div>
  </div>
</div>
