<script lang="ts">
  import { Dialog } from "bits-ui";
  import { page } from "$app/state";
  import { replaceState } from "$app/navigation";
  import { deLocalizeUrl } from "$lib/paraglide/runtime";
  import { translationSegmentsFromId } from "$lib/data/quran";
  import { STACKED_MAX_EXTRAS } from "$lib/data/quran-types";
  import type { TranslationCatalogueEntry } from "$lib/data/quran-types";
  import { withMoreParam } from "$lib/reader/more-param";
  import {
    TRANSLATION_CATALOGUE,
    TRANSLATION_CATALOGUE_BY_ID,
    flagFor,
    translationSourceOf,
  } from "$lib/quran/catalogue";
  import type { TranslationProvenance } from "$lib/quran/catalogue";
  import { stackedTranslations } from "$lib/stores/stacked-translations.svelte";
  import { reader } from "$lib/stores/reader.svelte";
  import { readerSource } from "$lib/stores/reader-settings.svelte";
  import { getReaderUiCopy } from "$lib/i18n/reader-copy";
  import { readerHrefFor } from "$lib/i18n/reader";
  import { publicHref } from "$lib/i18n/public-href";
  import { noteTranslationChosen } from "$lib/quran/engagement";
  import { Icon } from "$lib/components/icon";
  import {
    Tooltip,
    TooltipContent,
    TooltipProvider,
    TooltipTrigger,
  } from "$lib/components/ui/tooltip";
  import { hrefFor, positionOf } from "./translation-nav";
  import { translationMatchesQuery } from "./translation-search";

  type LanguageGroup = {
    language: string;
    flag: string;
    entries: TranslationCatalogueEntry[];
  };

  // Distinct color identity per provenance dot; fixed palette reads on the
  // inverted (bg-foreground) tooltip surface in both light and dark themes.
  const PROVENANCE_DOT = {
    qul: "bg-violet-500",
    quranenc: "bg-sky-500",
    tanzil: "bg-emerald-500",
  } satisfies Record<TranslationProvenance, string>;

  let { open = $bindable(false), primaryId = null }: { open?: boolean; primaryId?: string | null } =
    $props();

  const copy = getReaderUiCopy();
  let searchQuery = $state("");
  // Selected language in the rail; null until the modal first opens.
  let railLanguage = $state<string | null>(null);
  // Mobile only: true once a language is tapped, showing its pane over the rail.
  let mobilePane = $state(false);

  // Reset transient navigation state whenever the modal closes.
  $effect(() => {
    if (!open) {
      searchQuery = "";
      railLanguage = null;
      mobilePane = false;
    }
  });

  // Auto-select the primary translation's language on open: the reader's own
  // language is front and center instead of buried at its alphabetical spot.
  $effect(() => {
    if (!open || railLanguage !== null) return;
    const primaryEntry =
      primaryId !== null ? TRANSLATION_CATALOGUE_BY_ID.get(primaryId) : undefined;
    railLanguage = primaryEntry?.language ?? languages[0]?.language ?? null;
  });

  const position = $derived(positionOf(deLocalizeUrl(page.url).pathname));
  const selectedIds = $derived(stackedTranslations.ids);
  const isFull = $derived(selectedIds.length >= STACKED_MAX_EXTRAS);
  const searchActive = $derived(searchQuery.trim().length > 0);

  const matches = $derived.by(() => {
    const q = searchQuery.trim();
    if (!q) return TRANSLATION_CATALOGUE;
    return TRANSLATION_CATALOGUE.filter((t) =>
      translationMatchesQuery(q, {
        name: t.name,
        translator: t.translator,
        language: t.language,
        languageCode: t.languageCode,
        country: flagFor(t.languageCode).country,
      }),
    );
  });

  // Base-sensitivity collation keeps diacritic-laden language names (e.g.
  // future "Fātiḥah"-style labels) sorted next to their plain spellings.
  const languageCollator = new Intl.Collator("en", { sensitivity: "base" });

  function buildGroups(rows: readonly TranslationCatalogueEntry[]): LanguageGroup[] {
    const map = new Map<string, TranslationCatalogueEntry[]>();
    for (const t of rows) {
      const arr = map.get(t.language);
      if (arr) arr.push(t);
      else map.set(t.language, [t]);
    }
    return [...map.entries()]
      .map(([language, entries]) => ({
        language,
        flag: flagFor(entries[0]?.languageCode ?? "").flag,
        entries: [...entries].sort((a, b) => languageCollator.compare(a.name, b.name)),
      }))
      .sort((a, b) => languageCollator.compare(a.language, b.language));
  }

  // The rail shows matching languages only while searching, so language hits
  // (e.g. "urdu") narrow the rail as well as the flat result list.
  const languages = $derived(buildGroups(searchActive ? matches : TRANSLATION_CATALOGUE));

  const activeLanguage = $derived.by(() => {
    if (railLanguage !== null && languages.some((l) => l.language === railLanguage)) {
      return railLanguage;
    }
    return languages[0]?.language ?? null;
  });

  // Keep the selected rail row in view: on open the auto-selected primary
  // language can sit far below the fold, and search filtering can shrink the
  // rail around it. "nearest" never scrolls when the row is already visible.
  $effect(() => {
    if (!open || activeLanguage === null) return;
    const row = document.querySelector<HTMLElement>(
      `[data-language-option="${CSS.escape(activeLanguage)}"]`,
    );
    // jsdom defines no scrollIntoView; presence-check so tests never call it.
    if (row !== null && "scrollIntoView" in row) {
      row.scrollIntoView({ block: "nearest" });
    }
  });

  // Flat cross-language rows while searching, ordered by language then name.
  const searchRows = $derived.by(() => {
    if (!searchActive) return [];
    return [...matches].sort(
      (a, b) =>
        languageCollator.compare(a.language, b.language) ||
        languageCollator.compare(a.name, b.name),
    );
  });

  const activeEntries = $derived.by(() => {
    const group = languages.find((l) => l.language === activeLanguage);
    return group?.entries ?? [];
  });

  // Selected chips: route primary first (badged), then stacked extras in store
  // order. Updates live from store/prop changes; the row is absent when empty.
  const selectedRows = $derived.by(() => {
    const rows: TranslationCatalogueEntry[] = [];
    const primaryEntry =
      primaryId !== null ? TRANSLATION_CATALOGUE_BY_ID.get(primaryId) : undefined;
    if (primaryEntry) rows.push(primaryEntry);
    for (const id of selectedIds) {
      if (id === primaryId) continue;
      const found = TRANSLATION_CATALOGUE_BY_ID.get(id);
      if (found) rows.push(found);
    }
    return rows;
  });

  // Mobile: the pane replaces the rail once a language is chosen; searching
  // always shows the pane (the rail is filtered anyway). md+ shows both.
  const paneVisible = $derived(mobilePane || searchActive);

  // Selection changes only touch the stacked store + the ?more= URL param; the
  // reader layout's effects keep worker pinning in sync from there.
  function syncUrl(): void {
    replaceState(withMoreParam(page.url, stackedTranslations.ids), page.state);
  }
  function toggle(id: string): void {
    stackedTranslations.toggle(id);
    syncUrl();
  }
  function reorder(id: string, delta: -1 | 1): void {
    stackedTranslations.reorder(id, delta);
    syncUrl();
  }
  function remove(id: string): void {
    stackedTranslations.remove(id);
    syncUrl();
  }
  function clear(): void {
    stackedTranslations.clear();
    syncUrl();
  }

  function rowLabel(t: TranslationCatalogueEntry): string {
    return t.translator ?? t.name;
  }
  // The author line exists only when it adds information; catalogue rows whose
  // translator mirrors the name show a single line (no verbatim duplication).
  function hasAuthorLine(t: TranslationCatalogueEntry): boolean {
    if (t.translator === null) return false;
    return t.translator.trim().toLowerCase() !== t.name.trim().toLowerCase();
  }
  function rowHref(t: TranslationCatalogueEntry): `/app/${string}` | null {
    const seg = translationSegmentsFromId(t.id);
    return hrefFor(position, { id: t.id, lang: seg.lang, translator: seg.translator });
  }
  function onPrimary(t: TranslationCatalogueEntry): void {
    readerSource.setSourceId(t.id);
    void noteTranslationChosen(t.id);
    open = false;
  }
  function formatSize(bytes: number): string {
    if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${Math.max(1, Math.round(bytes / 1024))} KB`;
  }

  function selectLanguage(language: string): void {
    railLanguage = language;
    mobilePane = true;
  }
  function backToRail(): void {
    if (searchActive) searchQuery = "";
    mobilePane = false;
  }

  // Roving-tabindex keyboard support: arrows move the language selection (and
  // focus) within the rail. This is intrinsic widget navigation, not an app
  // shortcut, so it lives on the option buttons themselves.
  function onRailKeydown(event: KeyboardEvent, index: number): void {
    let next: number;
    if (event.key === "ArrowDown") next = (index + 1) % languages.length;
    else if (event.key === "ArrowUp") next = (index - 1 + languages.length) % languages.length;
    else return;
    event.preventDefault();
    const language = languages[next]?.language;
    if (language === undefined) return;
    railLanguage = language;
    // All rail options are already in the DOM, so focus can move synchronously
    // without waiting for the selection state to re-render.
    document
      .querySelector<HTMLElement>(`[data-language-option="${CSS.escape(language)}"]`)
      ?.focus();
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Portal>
    <Dialog.Overlay class="fixed inset-0 z-50 bg-black/40 backdrop-blur-[1px]" />
    <Dialog.Content
      class="fixed left-1/2 top-1/2 z-50 flex max-h-[85vh] w-[min(94vw,780px)] -translate-x-1/2 -translate-y-1/2 flex-col overflow-hidden rounded-xl border border-border bg-popover bg-clip-padding text-popover-foreground shadow-lg"
    >
      <div class="flex items-center justify-between gap-3 px-5 pb-3 pt-4">
        <Dialog.Title class="text-[17px] font-semibold leading-tight">
          {copy.stacked.title}
        </Dialog.Title>
        <Dialog.Description class="sr-only">{copy.translations.description}</Dialog.Description>
        <Dialog.Close>
          {#snippet child({ props })}
            <button
              {...props}
              type="button"
              aria-label={copy.translations.close}
              class="flex h-8 w-8 flex-none cursor-pointer items-center justify-center rounded-lg text-muted-foreground transition-colors hover:bg-surface-hover hover:text-foreground"
            >
              <Icon name="x" size={15} />
            </button>
          {/snippet}
        </Dialog.Close>
      </div>

      <div class="px-5">
        <div
          class="flex h-10 items-center gap-2.5 rounded-lg border border-border bg-background-subtle px-3.5 transition-colors focus-within:border-border-strong"
        >
          <Icon name="search" size={15} class="flex-none text-muted-foreground" />
          <input
            type="search"
            value={searchQuery}
            oninput={(e) => (searchQuery = e.currentTarget.value)}
            placeholder={copy.stacked.searchPlaceholder}
            aria-label={copy.stacked.searchPlaceholder}
            class="h-auto flex-1 border-0 bg-transparent px-0 py-0 text-sm text-foreground shadow-none outline-none placeholder:text-muted-foreground focus-visible:outline-none [&::-webkit-search-cancel-button]:hidden"
          />
          {#if searchQuery !== ""}
            <button
              type="button"
              onclick={() => (searchQuery = "")}
              aria-label={copy.sidebar.clearSearch}
              class="flex h-6 w-6 flex-none cursor-pointer items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-surface-hover hover:text-foreground"
            >
              <Icon name="x" size={13} />
            </button>
          {/if}
        </div>
      </div>

      <TooltipProvider delayDuration={300}>
        {#if reader.isVerseMode && selectedRows.length > 0}
          <div data-selected-chips class="flex items-start justify-between gap-3 px-5 pt-3">
            <div class="flex min-w-0 flex-1 flex-wrap items-center gap-1.5">
              {#each selectedRows as t (t.id)}
                {@const isPrimary = t.id === primaryId}
                {@const extraIndex = selectedIds.indexOf(t.id)}
                <div
                  data-chip={t.id}
                  class="group/chip inline-flex h-8 max-w-full items-center gap-1 rounded-pill border border-border bg-background-subtle pe-0.5 ps-2.5 transition-colors hover:border-border-strong"
                >
                  <span class="flex-none text-xs leading-none" aria-hidden="true">
                    {flagFor(t.languageCode).flag}
                  </span>
                  {#if isPrimary}
                    <Tooltip>
                      <TooltipTrigger>
                        {#snippet child({ props })}
                          <button
                            {...props}
                            type="button"
                            aria-label={copy.translations.primaryTip}
                            class="flex h-8 cursor-help items-center rounded-pill"
                          >
                            <span
                              class="rounded-pill bg-primary px-1.5 py-0.5 text-[10px] font-semibold text-primary-foreground"
                            >
                              {copy.stacked.primaryBadge}
                            </span>
                          </button>
                        {/snippet}
                      </TooltipTrigger>
                      <TooltipContent
                        class="max-w-[220px] rounded-md px-3 py-2 text-[12px] leading-snug whitespace-normal"
                      >
                        {copy.translations.primaryTip}
                      </TooltipContent>
                    </Tooltip>
                  {/if}
                  <span class="min-w-0 truncate text-[13px] font-medium text-foreground">
                    {t.name}
                  </span>
                  {#if !isPrimary}
                    <button
                      type="button"
                      onclick={() => reorder(t.id, -1)}
                      disabled={extraIndex <= 0}
                      aria-label={copy.stacked.moveUp}
                      class="flex h-8 w-7 flex-none cursor-pointer items-center justify-center rounded-md text-muted-foreground opacity-0 transition-opacity hover:text-foreground focus-visible:opacity-100 group-hover/chip:opacity-100 disabled:cursor-not-allowed disabled:opacity-30"
                    >
                      <Icon name="arrow-right" size={12} class="-rotate-90" />
                    </button>
                    <button
                      type="button"
                      onclick={() => reorder(t.id, 1)}
                      disabled={extraIndex === -1 || extraIndex >= selectedIds.length - 1}
                      aria-label={copy.stacked.moveDown}
                      class="flex h-8 w-7 flex-none cursor-pointer items-center justify-center rounded-md text-muted-foreground opacity-0 transition-opacity hover:text-foreground focus-visible:opacity-100 group-hover/chip:opacity-100 disabled:cursor-not-allowed disabled:opacity-30"
                    >
                      <Icon name="arrow-right" size={12} class="rotate-90" />
                    </button>
                  {/if}
                  <button
                    type="button"
                    onclick={() => remove(t.id)}
                    aria-label={copy.stacked.remove}
                    class="flex h-8 w-7 flex-none cursor-pointer items-center justify-center rounded-md text-muted-foreground transition-colors hover:text-foreground"
                  >
                    <Icon name="x" size={13} />
                  </button>
                </div>
              {/each}
            </div>
            <div class="flex flex-none items-center gap-2.5 pt-2 text-xs text-muted-foreground">
              <span
                class="tabular-nums"
                title={copy.translations.capNote(STACKED_MAX_EXTRAS)}
              >
                {copy.stacked.count(selectedIds.length, STACKED_MAX_EXTRAS)}
              </span>
              {#if selectedIds.length > 0}
                <button
                  type="button"
                  onclick={clear}
                  class="cursor-pointer transition-colors hover:text-foreground"
                >
                  {copy.stacked.clear}
                </button>
              {/if}
            </div>
          </div>
        {/if}

        <div
          class="grid min-h-0 flex-1 overflow-hidden border-t border-border md:grid-cols-[260px_1fr]"
        >
          {#snippet translationRow(t: TranslationCatalogueEntry, withLanguage: boolean)}
          {@const checked = selectedIds.includes(t.id)}
          {@const isPrimary = t.id === primaryId}
          {@const disabled = !reader.isVerseMode || isPrimary || (isFull && !checked)}
          {@const href = rowHref(t)}
          <li
            data-translation-row={t.id}
            class="flex items-center gap-3 rounded-lg px-3 py-2 transition-colors {checked
              ? 'bg-primary/10'
              : 'hover:bg-surface-hover'}"
          >
            {#if withLanguage}
              <span
                data-row-language
                class="w-[88px] flex-none truncate text-xs text-muted-foreground"
              >
                <span aria-hidden="true">{flagFor(t.languageCode).flag}</span>
                {t.language}
              </span>
            {/if}
            {#if isPrimary}
              <span class="flex w-14 flex-none items-center">
                <span
                  class="rounded-pill bg-primary/10 px-1.5 py-0.5 text-[10px] font-semibold text-primary"
                >
                  {copy.stacked.primaryBadge}
                </span>
              </span>
            {:else}
              <span class="flex w-14 flex-none items-center">
                <input
                  id={`tmodal-${t.id}`}
                  type="checkbox"
                  checked={checked}
                  disabled={disabled}
                  onchange={() => toggle(t.id)}
                  aria-label={rowLabel(t)}
                  class="size-[18px] flex-none cursor-pointer accent-primary disabled:cursor-not-allowed"
                />
              </span>
            {/if}
            {#if href}
              <a
                href={publicHref(readerHrefFor(copy.locale, href))}
                data-switch
                data-sveltekit-preload-data="hover"
                onclick={() => onPrimary(t)}
                aria-label={`${copy.translations.switchTo}: ${rowLabel(t)}`}
                title={copy.translations.switchTo}
                class="min-w-0 flex-1 cursor-pointer py-0.5"
              >
                <span class="block truncate text-sm font-medium text-foreground">
                  {t.name}
                </span>
                {#if hasAuthorLine(t)}
                  <span
                    data-author-line
                    class="block truncate text-[12.5px] leading-snug text-muted-foreground"
                  >
                    {t.translator}
                  </span>
                {/if}
              </a>
            {:else}
              <span class="min-w-0 flex-1 cursor-default py-0.5">
                <span class="block truncate text-sm font-medium text-foreground">
                  {t.name}
                </span>
                {#if hasAuthorLine(t)}
                  <span
                    data-author-line
                    class="block truncate text-[12.5px] leading-snug text-muted-foreground"
                  >
                    {t.translator}
                  </span>
                {/if}
              </span>
            {/if}
            <Tooltip>
              <TooltipTrigger>
                {#snippet child({ props })}
                  <button
                    {...props}
                    type="button"
                    aria-label={copy.translations.sourceLabel(translationSourceOf(t.id))}
                    class="flex h-7 w-7 flex-none cursor-pointer items-center justify-center rounded-md opacity-70 transition-opacity hover:opacity-100"
                  >
                    <span
                      class="size-2 flex-none rounded-full {PROVENANCE_DOT[translationSourceOf(t.id)]}"
                      aria-hidden="true"
                    ></span>
                  </button>
                {/snippet}
              </TooltipTrigger>
              <TooltipContent
                class="flex w-[260px] max-w-[260px] flex-col items-start gap-1.5 whitespace-normal rounded-md px-3 py-2.5 text-start leading-snug"
              >
                <span class="text-[12px] font-semibold">{t.name}</span>
                <span
                  class="inline-flex items-center gap-1.5 rounded-pill bg-background/15 px-2 py-0.5 text-[11px] font-medium"
                >
                  <span
                    class="size-1.5 flex-none rounded-full {PROVENANCE_DOT[translationSourceOf(t.id)]}"
                    aria-hidden="true"
                  ></span>
                  {copy.translations.sourceLabel(translationSourceOf(t.id))}
                </span>
                <dl class="flex w-full flex-col gap-0.5 text-[11px]">
                  {#if t.translator !== null}
                    <div class="flex w-full gap-2">
                      <dt class="w-[4.5rem] flex-none text-background/60">
                        {copy.translations.tooltipTranslator}
                      </dt>
                      <dd class="min-w-0 flex-1">{t.translator}</dd>
                    </div>
                  {/if}
                  <div class="flex w-full gap-2">
                    <dt class="w-[4.5rem] flex-none text-background/60">
                      {copy.translations.tooltipLanguage}
                    </dt>
                    <dd class="min-w-0 flex-1">
                      <span aria-hidden="true">{flagFor(t.languageCode).flag}</span>
                      {t.language}
                    </dd>
                  </div>
                  <div class="flex w-full gap-2">
                    <dt class="w-[4.5rem] flex-none text-background/60">
                      {copy.translations.tooltipSize}
                    </dt>
                    <dd class="min-w-0 flex-1">{formatSize(t.sizeBytes)}</dd>
                  </div>
                  <div class="flex w-full gap-2">
                    <dt class="w-[4.5rem] flex-none text-background/60">
                      {copy.translations.tooltipDirection}
                    </dt>
                    <dd class="min-w-0 flex-1">{copy.translations.dirLabel(t.direction)}</dd>
                  </div>
                </dl>
              </TooltipContent>
            </Tooltip>
          </li>
          {/snippet}
          <nav
            data-language-rail
            aria-label={copy.translations.languagesLabel}
            class="{paneVisible ? 'hidden' : 'flex'} md:flex min-h-0 flex-col gap-0.5 overflow-y-auto border-border p-2 md:border-e"
          >
            {#each languages as l, i (l.language)}
              {@const active = l.language === activeLanguage}
              <button
                data-language-option={l.language}
                type="button"
                tabindex={active ? 0 : -1}
                aria-current={active ? "true" : undefined}
                onclick={() => selectLanguage(l.language)}
                onkeydown={(e) => onRailKeydown(e, i)}
                class="flex h-10 cursor-pointer items-center gap-2.5 rounded-lg px-2.5 text-start text-sm transition-colors {active
                  ? 'bg-primary/10 font-medium text-foreground'
                  : 'text-foreground-secondary hover:bg-surface-hover hover:text-foreground'}"
              >
                <span class="flex-none text-sm leading-none" aria-hidden="true">{l.flag}</span>
                <span class="min-w-0 flex-1 truncate">{l.language}</span>
                <span class="flex-none text-xs tabular-nums text-muted-foreground">
                  {l.entries.length}
                </span>
              </button>
            {/each}
          </nav>

          <section
            data-language-pane
            class="{paneVisible ? 'flex' : 'hidden'} md:flex min-h-0 flex-1 flex-col overflow-y-auto"
          >
            {#if reader.isReadingMode}
              <p
                class="mx-3 mt-3 rounded-lg border border-border bg-background-subtle px-3.5 py-2.5 text-[13px] leading-relaxed text-foreground-secondary"
              >
                {copy.translations.readingNotice}
              </p>
            {/if}

            {#if searchActive}
              <div class="flex items-center gap-2 px-4 pb-1 pt-3">
                <button
                  type="button"
                  onclick={backToRail}
                  aria-label={copy.translations.back}
                  class="flex h-8 -ms-2 flex-none cursor-pointer items-center gap-1 rounded-lg px-2 text-sm text-muted-foreground transition-colors hover:text-foreground md:hidden"
                >
                  <Icon name="arrow-right" size={14} class="rotate-180" />
                  {copy.translations.back}
                </button>
                <p data-results-count class="text-[13px] text-muted-foreground">
                  {copy.translations.results(matches.length)}
                </p>
              </div>
              {#if matches.length === 0}
                <p class="px-4 py-3 text-sm text-muted-foreground" role="status">
                  {copy.translations.noMatches}
                </p>
              {:else}
                <ul class="flex flex-col gap-0.5 px-2 pb-4">
                  {#each searchRows as t (t.id)}
                    {@render translationRow(t, true)}
                  {/each}
                </ul>
              {/if}
            {:else if activeLanguage !== null}
              <div class="flex items-center gap-2 px-4 pb-1 pt-3">
                <button
                  type="button"
                  onclick={backToRail}
                  aria-label={copy.translations.back}
                  class="flex h-8 -ms-2 flex-none cursor-pointer items-center gap-1 rounded-lg px-2 text-sm text-muted-foreground transition-colors hover:text-foreground md:hidden"
                >
                  <Icon name="arrow-right" size={14} class="rotate-180" />
                  {copy.translations.back}
                </button>
                <h3 class="text-[15px] font-semibold text-foreground">{activeLanguage}</h3>
                <span data-results-count class="text-[13px] text-muted-foreground">
                  {copy.translations.results(activeEntries.length)}
                </span>
              </div>
              <ul class="flex flex-col gap-0.5 px-2 pb-4">
                {#each activeEntries as t (t.id)}
                  {@render translationRow(t, false)}
                {/each}
              </ul>
            {/if}
          </section>
        </div>
      </TooltipProvider>

      <div class="flex items-center justify-between gap-3 border-t border-border px-5 py-3">
        <p data-cap-note class="text-xs text-muted-foreground">
          {reader.isVerseMode && isFull
            ? copy.stacked.full(STACKED_MAX_EXTRAS)
            : copy.translations.capNote(STACKED_MAX_EXTRAS)}
        </p>
        <button
          type="button"
          data-done
          onclick={() => (open = false)}
          class="flex h-9 flex-none cursor-pointer items-center rounded-lg bg-primary px-5 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary-hover"
        >
          {copy.translations.done}
        </button>
      </div>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
