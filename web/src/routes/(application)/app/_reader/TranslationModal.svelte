<script lang="ts">
  import { Dialog } from "bits-ui";
  import { page } from "$app/state";
  import { replaceState } from "$app/navigation";
  import { deLocalizeUrl } from "$lib/paraglide/runtime";
  import {
    translationSegmentsFromId,
  } from "$lib/data/quran";
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

  type GroupedLang = {
    language: string;
    flag: string;
    entries: TranslationCatalogueEntry[];
  };

  // Distinct color identity per provenance chip; fixed palette dots read on the
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

  $effect(() => {
    if (!open) searchQuery = "";
  });

  const position = $derived(positionOf(deLocalizeUrl(page.url).pathname));
  const selectedIds = $derived(stackedTranslations.ids);
  const isFull = $derived(selectedIds.length >= STACKED_MAX_EXTRAS);

  const filtered = $derived.by(() => {
    const q = searchQuery.trim();
    const list = TRANSLATION_CATALOGUE;
    if (!q) return list;
    return list.filter((t) =>
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

  const grouped = $derived.by<GroupedLang[]>(() => {
    const map = new Map<string, TranslationCatalogueEntry[]>();
    for (const t of filtered) {
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
  });

  const selectedEntries = $derived.by(() => {
    const out: TranslationCatalogueEntry[] = [];
    for (const id of selectedIds) {
      const found = TRANSLATION_CATALOGUE_BY_ID.get(id);
      if (found) out.push(found);
    }
    return out;
  });

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
  function selectedLabel(t: TranslationCatalogueEntry): string {
    return t.translator ? `${t.language} · ${t.translator}` : t.language;
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
</script>

<Dialog.Root bind:open>
  <Dialog.Portal>
    <Dialog.Overlay class="fixed inset-0 z-50 bg-black/40 backdrop-blur-[1px]" />
    <Dialog.Content
      class="fixed left-1/2 top-1/2 z-50 flex max-h-[85vh] w-[min(92vw,560px)] -translate-x-1/2 -translate-y-1/2 flex-col gap-3 rounded-md border border-border bg-popover bg-clip-padding p-4 text-sm text-popover-foreground shadow-lg"
    >
      <div class="flex items-start justify-between gap-2">
        <div class="flex flex-col gap-0.5">
          <Dialog.Title class="text-base font-semibold">{copy.stacked.title}</Dialog.Title>
          <Dialog.Description class="sr-only">{copy.translations.description}</Dialog.Description>
        </div>
        <Dialog.Close>
          {#snippet child({ props })}
            <button
              {...props}
              type="button"
              aria-label={copy.translations.close}
              class="flex h-7 w-7 flex-none items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-surface-hover hover:text-foreground"
            >
              <Icon name="x" size={14} />
            </button>
          {/snippet}
        </Dialog.Close>
      </div>

      {#if reader.isReadingMode}
        <p class="rounded-md border border-border bg-background-subtle px-3 py-2 text-[12.5px] text-foreground-secondary">
          {copy.translations.readingNotice}
        </p>
      {/if}

      <div class="flex flex-col gap-2">
        <div class="flex items-center justify-between">
          <span class="text-[11.5px] text-muted-foreground">
            {copy.stacked.count(selectedIds.length, STACKED_MAX_EXTRAS)}
          </span>
          {#if reader.isVerseMode && selectedIds.length > 0}
            <button
              type="button"
              onclick={clear}
              aria-label={copy.stacked.clear}
              class="flex items-center gap-1 text-[11.5px] text-muted-foreground transition-colors hover:text-foreground"
            >
              <Icon name="x" size={12} />
              {copy.stacked.clear}
            </button>
          {/if}
        </div>

        <div
          class="flex items-center gap-2 rounded-md border border-border bg-background-subtle px-3 py-2 transition-colors focus-within:border-border-strong"
        >
          <span class="sr-only">{copy.stacked.searchPlaceholder}</span>
          <Icon name="search" size={13} class="flex-none text-muted-foreground" />
          <input
            type="search"
            value={searchQuery}
            oninput={(e) => (searchQuery = e.currentTarget.value)}
            placeholder={copy.stacked.searchPlaceholder}
            aria-label={copy.stacked.searchPlaceholder}
            class="h-auto flex-1 border-0 bg-transparent px-0 py-0 text-[13px] text-foreground shadow-none placeholder:text-muted-foreground focus-visible:outline-none"
          />
        </div>

        {#if reader.isVerseMode && isFull}
          <p class="text-[11.5px] text-muted-foreground">{copy.stacked.full(STACKED_MAX_EXTRAS)}</p>
        {/if}
      </div>

      <TooltipProvider delayDuration={300}>
        <div class="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto pe-1">
          {#if reader.isVerseMode && selectedIds.length > 0}
            <section>
              <div class="px-1 py-1 text-[10.5px] uppercase tracking-wide text-muted-foreground">
                {copy.stacked.selected}
              </div>
              <ol class="flex flex-col gap-0.5">
                {#each selectedEntries as t, i (t.id)}
                  <li class="flex items-center gap-1 rounded-sm px-2 py-1.5 text-[12.5px] text-foreground-secondary">
                    <span class="w-4 flex-none text-[10.5px] text-muted-foreground">{i + 1}</span>
                    <span class="min-w-0 flex-1 truncate">{selectedLabel(t)}</span>
                    {#if t.id === primaryId}
                      <span
                        class="flex-none rounded-pill bg-surface-hover px-1.5 py-0.5 text-[10px] text-muted-foreground"
                      >
                        {copy.stacked.primaryBadge}
                      </span>
                    {/if}
                    <button
                      type="button"
                      onclick={() => reorder(t.id, -1)}
                      disabled={i === 0}
                      aria-label={copy.stacked.moveUp}
                      class="flex-none p-1 text-muted-foreground transition-colors hover:text-foreground disabled:opacity-50"
                    >
                      <Icon name="arrow-right" size={12} class="-rotate-90" />
                    </button>
                    <button
                      type="button"
                      onclick={() => reorder(t.id, 1)}
                      disabled={i === selectedEntries.length - 1}
                      aria-label={copy.stacked.moveDown}
                      class="flex-none p-1 text-muted-foreground transition-colors hover:text-foreground disabled:opacity-50"
                    >
                      <Icon name="arrow-right" size={12} class="rotate-90" />
                    </button>
                    <button
                      type="button"
                      onclick={() => remove(t.id)}
                      aria-label={copy.stacked.remove}
                      class="flex-none p-1 text-muted-foreground transition-colors hover:text-foreground"
                    >
                      <Icon name="x" size={12} />
                    </button>
                  </li>
                {/each}
              </ol>
            </section>
          {/if}

          {#if grouped.length === 0}
            <p class="px-2 py-2 text-[12px] text-muted-foreground" role="status">
              {copy.translations.noMatches}
            </p>
          {/if}

          {#each grouped as g (g.language)}
            <!-- All groups always rendered (no collapsing); the modal body scrolls. -->
            <section data-language={g.language} class="flex flex-col gap-0.5">
              <div
                class="sticky top-0 z-10 flex items-center gap-1.5 rounded-sm bg-popover px-1 py-1 text-[10.5px] uppercase tracking-wide text-muted-foreground"
              >
                <span class="text-[12px] leading-none normal-case" aria-hidden="true">{g.flag}</span>
                {g.language}
                <span class="lowercase tracking-normal">({g.entries.length})</span>
              </div>
              <ul class="flex flex-col gap-0.5">
                {#each g.entries as t (t.id)}
                  {@const checked = selectedIds.includes(t.id)}
                  {@const disabled =
                    !reader.isVerseMode || t.id === primaryId || (isFull && !checked)}
                  {@const href = rowHref(t)}
                  <li class="flex items-center gap-2 rounded-sm px-2 py-2 text-[12.5px] transition-colors text-foreground-secondary hover:bg-surface-hover hover:text-foreground">
                    {#if reader.isVerseMode}
                      <input
                        id={`tmodal-${t.id}`}
                        type="checkbox"
                        checked={checked}
                        disabled={disabled}
                        onchange={() => toggle(t.id)}
                        aria-label={rowLabel(t)}
                        class="h-4 w-4 flex-none accent-primary disabled:cursor-not-allowed"
                      />
                    {/if}
                    <Tooltip>
                      <TooltipTrigger>
                        {#snippet child({ props })}
                          <span
                            {...props}
                            class="min-w-0 flex-1 truncate underline decoration-dotted decoration-muted-foreground underline-offset-2"
                          >
                            {rowLabel(t)}
                          </span>
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
                            <dd class="min-w-0 flex-1">
                              {copy.translations.dirLabel(t.direction)}
                            </dd>
                          </div>
                        </dl>
                      </TooltipContent>
                    </Tooltip>
                    {#if t.id === primaryId}
                      <span
                        class="flex-none rounded-pill bg-surface-hover px-1.5 py-0.5 text-[10px] text-muted-foreground"
                      >
                        {copy.stacked.primaryBadge}
                      </span>
                    {/if}
                    {#if href}
                      <a
                        href={publicHref(readerHrefFor(copy.locale, href))}
                        data-sveltekit-preload-data="hover"
                        onclick={() => onPrimary(t)}
                        aria-label={`${copy.translations.switchTo}: ${rowLabel(t)}`}
                        title={copy.translations.switchTo}
                        class="flex h-7 w-7 flex-none items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-surface-hover hover:text-foreground"
                      >
                        <Icon name="arrow-right" size={12} />
                      </a>
                    {/if}
                  </li>
                {/each}
              </ul>
            </section>
          {/each}
        </div>
      </TooltipProvider>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
