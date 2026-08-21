<script lang="ts">
  import { page } from "$app/state";
  import { replaceState } from "$app/navigation";
  import { stackedTranslations } from "$lib/stores/stacked-translations.svelte";
  import { STACKED_MAX_EXTRAS } from "$lib/data/quran-types";
  import type { TranslationCatalogueEntry } from "$lib/data/quran-types";
  import { withMoreParam } from "$lib/reader/more-param";
  import { TRANSLATION_CATALOGUE, TRANSLATION_CATALOGUE_BY_ID } from "$lib/quran/catalogue";
  import { translationIdFromSegments } from "$lib/data/quran";
  import { getReaderUiCopy } from "$lib/i18n/reader-copy";
  import { Icon } from "$lib/components/icon";
  import { cn } from "$lib/utils";
  import {
    Sheet,
    SheetTrigger,
    SheetContent,
    SheetHeader,
    SheetTitle,
  } from "$lib/components/ui/sheet";

  type GroupedLang = { language: string; entries: TranslationCatalogueEntry[] };

  const copy = getReaderUiCopy();

  let open = $state(false);
  let searchQuery = $state("");

  $effect(() => {
    if (!open) searchQuery = "";
  });

  const lang = $derived(page.params.lang);
  const translator = $derived(page.params.translator);
  const primaryId = $derived(
    lang && translator ? translationIdFromSegments(lang, translator) : null,
  );
  const selectedIds = $derived(stackedTranslations.ids);
  const isFull = $derived(selectedIds.length >= STACKED_MAX_EXTRAS);

  const filtered = $derived.by(() => {
    const q = searchQuery.trim().toLowerCase();
    const list = TRANSLATION_CATALOGUE;
    if (!q) return list;
    return list.filter(
      (t) =>
        t.language.toLowerCase().includes(q) ||
        (t.translator !== null && t.translator.toLowerCase().includes(q)) ||
        t.id.toLowerCase().includes(q),
    );
  });

  const grouped = $derived.by<GroupedLang[]>(() => {
    const map = new Map<string, TranslationCatalogueEntry[]>();
    for (const t of filtered) {
      const arr = map.get(t.language);
      if (arr) arr.push(t);
      else map.set(t.language, [t]);
    }
    return [...map.entries()].map(([language, entries]) => ({ language, entries }));
  });

  const selectedEntries = $derived.by(() => {
    const out: TranslationCatalogueEntry[] = [];
    for (const id of selectedIds) {
      const found = TRANSLATION_CATALOGUE_BY_ID.get(id);
      if (found) out.push(found);
    }
    return out;
  });

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
  function checkboxId(t: TranslationCatalogueEntry): string {
    return `stk-${t.id}`;
  }
</script>

<Sheet bind:open>
  <SheetTrigger
    class="flex h-9 w-9 flex-none items-center justify-center rounded-[9px] border border-line bg-bg-2 text-fg-2 transition-colors hover:border-line-3 hover:text-fg"
    aria-label={copy.stacked.open}
  >
    <Icon name="rows" size={15} class="text-fg-3" />
  </SheetTrigger>

  <SheetContent side="right" class="gap-3">
    <SheetHeader>
      <SheetTitle>{copy.stacked.title}</SheetTitle>
    </SheetHeader>

    <div class="flex flex-col gap-2 px-4">
      <div class="flex items-center justify-between">
        <span class="text-[11.5px] text-fg-3">
          {copy.stacked.count(selectedIds.length, STACKED_MAX_EXTRAS)}
        </span>
        {#if selectedIds.length > 0}
          <button
            type="button"
            onclick={clear}
            aria-label={copy.stacked.clear}
            class="text-fg-3 transition-colors hover:text-fg"
          >
            <Icon name="x" size={13} />
          </button>
        {/if}
      </div>

      <label
        class="flex items-center gap-2 rounded-[9px] border border-line bg-bg-2 px-3 py-2 transition-colors focus-within:border-line-3 focus-within:ring-2 focus-within:ring-accent/40"
      >
        <span class="sr-only">{copy.stacked.searchPlaceholder}</span>
        <Icon name="search" size={13} class="flex-none text-fg-3" />
        <input
          type="search"
          value={searchQuery}
          oninput={(e) => (searchQuery = e.currentTarget.value)}
          placeholder={copy.stacked.searchPlaceholder}
          aria-label={copy.stacked.searchPlaceholder}
          class="h-auto flex-1 border-0 bg-transparent px-0 py-0 text-[13px] text-fg shadow-none outline-none placeholder:text-fg-3 focus:ring-0"
        />
      </label>

      {#if isFull}
        <p class="text-[11.5px] text-fg-3">{copy.stacked.full(STACKED_MAX_EXTRAS)}</p>
      {/if}
    </div>

    <div class="flex flex-1 flex-col gap-3 overflow-y-auto px-4 pb-4">
      {#if selectedIds.length > 0}
        <section>
          <div class="px-1 py-1 text-[10.5px] uppercase tracking-wide text-fg-4">
            {copy.stacked.selected}
          </div>
          <ol class="flex flex-col gap-0.5">
            {#each selectedEntries as t, i (t.id)}
              <li class="flex items-center gap-1 rounded-[7px] px-2 py-1.5 text-[12.5px] text-fg-2">
                <span class="w-4 flex-none text-[10.5px] text-fg-4">{i + 1}</span>
                <span class="min-w-0 flex-1 truncate">{selectedLabel(t)}</span>
                {#if t.id === primaryId}
                  <span class="flex-none rounded-full bg-bg-3 px-1.5 py-0.5 text-[10px] text-fg-3">
                    {copy.stacked.primaryBadge}
                  </span>
                {/if}
                <button
                  type="button"
                  onclick={() => reorder(t.id, -1)}
                  disabled={i === 0}
                  aria-label={copy.stacked.moveUp}
                  class="flex-none p-1 text-fg-3 transition-colors hover:text-fg disabled:opacity-50"
                >
                  <Icon name="arrow-right" size={12} class="-rotate-90" />
                </button>
                <button
                  type="button"
                  onclick={() => reorder(t.id, 1)}
                  disabled={i === selectedEntries.length - 1}
                  aria-label={copy.stacked.moveDown}
                  class="flex-none p-1 text-fg-3 transition-colors hover:text-fg disabled:opacity-50"
                >
                  <Icon name="arrow-right" size={12} class="rotate-90" />
                </button>
                <button
                  type="button"
                  onclick={() => remove(t.id)}
                  aria-label={copy.stacked.remove}
                  class="flex-none p-1 text-fg-3 transition-colors hover:text-fg"
                >
                  <Icon name="x" size={12} />
                </button>
              </li>
            {/each}
          </ol>
        </section>
      {:else}
        <p class="px-2 py-2 text-[12px] text-fg-3" role="status">
          {copy.stacked.noneSelected}
        </p>
      {/if}

      {#each grouped as g (g.language)}
        <section>
          <div class="px-1 py-1 text-[10.5px] uppercase tracking-wide text-fg-4">
            {g.language}
          </div>
          <ul class="flex flex-col gap-0.5">
            {#each g.entries as t (t.id)}
              {@const checked = selectedIds.includes(t.id)}
              {@const disabled = t.id === primaryId || (isFull && !checked)}
              <li>
                <label
                  for={checkboxId(t)}
                  class={cn(
                    "flex items-center gap-2 rounded-[7px] px-3 py-2 text-[12.5px] transition-colors",
                    disabled
                      ? "cursor-not-allowed text-fg-4 opacity-60"
                      : "text-fg-2 hover:bg-bg-2 hover:text-fg",
                  )}
                >
                  <input
                    id={checkboxId(t)}
                    type="checkbox"
                    checked={checked}
                    disabled={disabled}
                    onchange={() => toggle(t.id)}
                    class="h-4 w-4 flex-none accent-[var(--accent)]"
                  />
                  <span class="min-w-0 flex-1 truncate">{rowLabel(t)}</span>
                  {#if t.id === primaryId}
                    <span class="flex-none rounded-full bg-bg-3 px-1.5 py-0.5 text-[10px] text-fg-3">
                      {copy.stacked.primaryBadge}
                    </span>
                  {/if}
                </label>
              </li>
            {/each}
          </ul>
        </section>
      {/each}
    </div>
  </SheetContent>
</Sheet>
