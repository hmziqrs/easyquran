<script lang="ts">
  import { Dialog } from "bits-ui";
  import { onMount } from "svelte";
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
    nativeNameFor,
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
    code: string;
    flag: string;
    autonym: string | null;
    entries: TranslationCatalogueEntry[];
  };

  // Provenance color identity lives ONLY inside the rich row tooltip (U19):
  // the source chip's dot reads on the inverted (bg-foreground) surface in
  // both light and dark themes. Rows themselves carry no dot.
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

  // Client-only browser languages for the rail's priority sort (see
  // browserBoostCodes above). onMount never runs on the server, so the SSR
  // render stays alphabetical and hydration cannot mismatch.
  onMount(() => {
    browserBoostCodes = browserLanguageCodes(navigator.languages);
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
      .map(([language, entries]) => {
        const code = entries[0]?.languageCode ?? "";
        return {
          language,
          code,
          flag: flagFor(code).flag,
          autonym: nativeNameFor(code),
          entries: [...entries].sort((a, b) => languageCollator.compare(a.name, b.name)),
        };
      })
      .sort((a, b) => languageCollator.compare(a.language, b.language));
  }

  // Browser-language boost for the rail's priority order (U21). Read in
  // onMount only: navigator does not exist during SSR, and assigning it there
  // keeps the server render alphabetical — the derived below re-sorts after
  // hydration with zero mismatch risk (same mounted-gate approach as
  // ReaderShell's client-only state).
  let browserBoostCodes = $state.raw<string[]>([]);

  // navigator.languages → deduped base language codes ("ur-PK" → "ur").
  // Unknown codes simply never match a group, so no catalogue filtering here.
  function browserLanguageCodes(languages: readonly string[]): string[] {
    const codes: string[] = [];
    for (const tag of languages) {
      const base = (tag.split("-")[0] ?? "").toLowerCase();
      if (base !== "" && !codes.includes(base)) codes.push(base);
    }
    return codes;
  }

  // Rail priority (U21): Arabic always first, English second, then the user's
  // browser languages in their stated preference order, then alphabetical.
  function railRank(group: LanguageGroup): number {
    if (group.code === "ar") return 0;
    if (group.code === "en") return 1;
    const boostIndex = browserBoostCodes.indexOf(group.code);
    if (boostIndex !== -1) return 2 + boostIndex;
    return Number.MAX_SAFE_INTEGER;
  }

  function compareRailGroups(a: LanguageGroup, b: LanguageGroup): number {
    const rankA = railRank(a);
    const rankB = railRank(b);
    if (rankA !== rankB) return rankA - rankB;
    return languageCollator.compare(a.language, b.language);
  }

  // The rail shows matching languages only while searching, so language hits
  // (e.g. "urdu") narrow the rail as well as the flat result list.
  const languages = $derived(
    buildGroups(searchActive ? matches : TRANSLATION_CATALOGUE).sort(compareRailGroups),
  );

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

  // Row cover classes (stress S9): one >=44px (min-h-11) cover per row, so the
  // checkbox zone and the switch link both sit on full-row targets. "toggle"
  // rows are labels whose whole surface toggles the checkbox; "locked" mirrors
  // a disabled checkbox (reading mode / cap); "static" covers primary rows,
  // which have no checkbox to associate (switching stays on the name link).
  // touch-manipulation kills the double-tap-zoom window without breaking the
  // pane's scroll (stress S13).
  function rowCoverClass(kind: "toggle" | "locked" | "static"): string {
    const base = "flex min-h-11 flex-1 touch-manipulation items-center gap-1.5 px-3";
    if (kind === "toggle") return `${base} cursor-pointer`;
    if (kind === "locked") return `${base} cursor-not-allowed`;
    return base;
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
      class="fixed left-1/2 top-1/2 z-50 flex max-h-[85vh] w-[min(94vw,780px)] -translate-x-1/2 -translate-y-1/2 flex-col overflow-hidden rounded-xl border border-border bg-popover bg-clip-padding pt-[env(safe-area-inset-top)] pb-[env(safe-area-inset-bottom)] ps-[env(safe-area-inset-left)] pe-[env(safe-area-inset-right)] text-popover-foreground shadow-lg"
    >
      <!-- S11 (stress A3): env(safe-area-inset-*) padding keeps modal content
           clear of notches/home indicators on devices that report insets. -->
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
              class="flex h-11 w-11 flex-none cursor-pointer touch-manipulation items-center justify-center rounded-lg text-muted-foreground transition-colors hover:bg-surface-hover hover:text-foreground"
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
            <!-- S9 (stress A1): the clear affordance stays visually small, but
                 its before-pseudo grows the hit target to 44px square. -->
            <button
              type="button"
              onclick={() => (searchQuery = "")}
              aria-label={copy.sidebar.clearSearch}
              class="relative flex h-7 w-7 flex-none cursor-pointer touch-manipulation items-center justify-center rounded-md text-muted-foreground transition-colors before:absolute before:-inset-2 before:content-[''] hover:bg-surface-hover hover:text-foreground"
            >
              <Icon name="x" size={13} />
            </button>
          {/if}
        </div>
      </div>

      <TooltipProvider delayDuration={300}>
        {#if reader.isVerseMode && selectedRows.length > 0}
          <div data-selected-chips class="flex items-start justify-between gap-3 px-5 pt-3">
            <!-- S14 (stress B1): a single horizontal scroll row, never a wrapped
                 chip wall — the strip costs one row max and keeps touch
                 momentum; overscroll-contain stops end-of-strip flicks from
                 chaining to the page behind the modal (S12). -->
            <div
              class="flex min-w-0 flex-1 touch-manipulation items-center gap-1.5 overflow-x-auto overscroll-contain"
            >
              {#each selectedRows as t (t.id)}
                {@const isPrimary = t.id === primaryId}
                {@const extraIndex = selectedIds.indexOf(t.id)}
                <div
                  data-chip={t.id}
                  class="group/chip inline-flex h-11 max-w-full flex-none touch-manipulation items-center gap-1 rounded-pill border border-border bg-background-subtle pe-0.5 ps-2.5 transition-colors hover:border-border-strong"
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
                            class="flex h-11 cursor-help touch-manipulation items-center rounded-pill"
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
                    <!-- S8/S9 (stress A2/A1): the hover-reveal compiles only
                         under @media(hover:hover), so coarse pointers get the
                         arrows always (via the hover:none variant), keyboard
                         focus-within on the chip reveals them too, and the
                         before-pseudo widens each 44px-tall control to a 44px
                         hit area without growing the chip. -->
                    <button
                      type="button"
                      onclick={() => reorder(t.id, -1)}
                      disabled={extraIndex <= 0}
                      aria-label={copy.stacked.moveUp}
                      class="relative flex h-11 w-7 flex-none cursor-pointer touch-manipulation items-center justify-center rounded-md text-muted-foreground opacity-0 transition-opacity before:absolute before:-inset-x-2 before:inset-y-0 before:content-[''] hover:text-foreground focus-visible:opacity-100 group-hover/chip:opacity-100 group-focus-within/chip:opacity-100 [@media(hover:none)]:enabled:opacity-100 disabled:cursor-not-allowed disabled:opacity-30"
                    >
                      <Icon name="arrow-right" size={12} class="-rotate-90" />
                    </button>
                    <button
                      type="button"
                      onclick={() => reorder(t.id, 1)}
                      disabled={extraIndex === -1 || extraIndex >= selectedIds.length - 1}
                      aria-label={copy.stacked.moveDown}
                      class="relative flex h-11 w-7 flex-none cursor-pointer touch-manipulation items-center justify-center rounded-md text-muted-foreground opacity-0 transition-opacity before:absolute before:-inset-x-2 before:inset-y-0 before:content-[''] hover:text-foreground focus-visible:opacity-100 group-hover/chip:opacity-100 group-focus-within/chip:opacity-100 [@media(hover:none)]:enabled:opacity-100 disabled:cursor-not-allowed disabled:opacity-30"
                    >
                      <Icon name="arrow-right" size={12} class="rotate-90" />
                    </button>
                  {/if}
                  <button
                    type="button"
                    onclick={() => remove(t.id)}
                    aria-label={copy.stacked.remove}
                    class="relative flex h-11 w-7 flex-none cursor-pointer touch-manipulation items-center justify-center rounded-md text-muted-foreground transition-colors before:absolute before:-inset-x-2 before:inset-y-0 before:content-[''] hover:text-foreground"
                  >
                    <Icon name="x" size={13} />
                  </button>
                </div>
              {/each}
            </div>
            <div
              class="flex flex-none self-stretch touch-manipulation items-center gap-2.5 text-xs text-muted-foreground"
            >
              <span
                class="tabular-nums"
                title={copy.translations.capNote(STACKED_MAX_EXTRAS)}
              >
                {copy.stacked.count(selectedIds.length, STACKED_MAX_EXTRAS)}
              </span>
              {#if selectedIds.length > 0}
                <!-- S9 (stress A1): Clear all is ~46x16 visually; real vertical
                     padding plus a before-pseudo give a >=44px effective
                     target with zero visible change (the button has no
                     background of its own). -->
                <button
                  type="button"
                  onclick={clear}
                  class="relative cursor-pointer touch-manipulation py-2.5 transition-colors before:absolute before:-inset-2 before:content-[''] hover:text-foreground"
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
          {@const native = nativeNameFor(t.languageCode)}
          <!-- U16: the rich tooltip triggers from the whole row. The trigger
               props land on the li via the child snippet; tabindex stays -1
               (the row itself is not a tab stop) and focus is forwarded with
               the bubbling focusin/focusout so keyboard focus on the row's
               checkbox/link opens the tooltip too. SAFETY: the child-snippet
               props bag is untyped, so the forwarded trigger handlers carry a
               FocusEvent-cast — they are bits-ui's own onfocus/onblur. -->
          <!-- S9/S10 (stress A1/A2): the row body is a <label> tied to the row
               checkbox, so checkbox + name + author + the touch-only source
               line form one >=44px toggle target; the name link stays a link
               (primary switch) but self-stretches to the full row height, so
               it also lands on a >=44px target. Primary rows keep a plain
               cover — no checkbox to associate. -->

          <Tooltip>
            <TooltipTrigger tabindex={-1}>
              {#snippet child({ props })}
                <!-- omit-pattern destructure: bits-ui merges a button-only
                     `type` into the trigger props; it is meaningless on an li
                     and must not reach the DOM. -->
                {@const { type: _triggerType, ...rowProps } = props}
                <li
                  {...rowProps}
                  onfocusin={rowProps.onfocus as ((event: FocusEvent) => void) | undefined}
                  onfocusout={rowProps.onblur as ((event: FocusEvent) => void) | undefined}
                  data-translation-row={t.id}
                  class="flex rounded-lg transition-colors {checked
                    ? 'bg-primary/10'
                    : 'hover:bg-surface-hover'}"
                >
                  {#snippet coverBody()}
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
                    <!-- spacer keeps the name column aligned with checkbox rows -->
                    <span class="w-[18px] flex-none" aria-hidden="true"></span>
                  {:else}
                    <input
                      id={`tmodal-${t.id}`}
                      type="checkbox"
                      checked={checked}
                      disabled={disabled}
                      onchange={() => toggle(t.id)}
                      aria-label={rowLabel(t)}
                      class="size-[18px] flex-none cursor-pointer touch-manipulation accent-primary disabled:cursor-not-allowed"
                    />
                  {/if}
                  {#if href}
                    <a
                      href={publicHref(readerHrefFor(copy.locale, href))}
                      data-switch
                      data-sveltekit-preload-data="hover"
                      onclick={() => onPrimary(t)}
                      aria-label={`${copy.translations.switchTo}: ${rowLabel(t)}`}
                      class="flex min-w-0 flex-1 cursor-pointer touch-manipulation flex-col justify-center self-stretch"
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
                      <!-- S10 (stress A2): provenance is hover-tooltip-only, so
                           coarse pointers get it as a muted text line instead
                           ([@media(hover:hover)]:hidden keeps hover devices on
                           the clean one/two-line row). -->
                      <span
                        data-row-source
                        class="block truncate text-[11px] leading-tight text-muted-foreground [@media(hover:hover)]:hidden"
                      >
                        {copy.translations.sourceLabel(translationSourceOf(t.id))}
                      </span>
                    </a>
                  {:else}
                    <span
                      class="flex min-w-0 flex-1 flex-col justify-center self-stretch"
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
                      <span
                        data-row-source
                        class="block truncate text-[11px] leading-tight text-muted-foreground [@media(hover:hover)]:hidden"
                      >
                        {copy.translations.sourceLabel(translationSourceOf(t.id))}
                      </span>
                    </span>
                  {/if}
                  {#if isPrimary}
                    <span
                      class="flex-none rounded-pill bg-primary/10 px-1.5 py-0.5 text-[10px] font-semibold text-primary"
                    >
                      {copy.stacked.primaryBadge}
                    </span>
                  {/if}
                  {/snippet}
                  {#if isPrimary}
                    <div data-row-cover class={rowCoverClass("static")}>
                      {@render coverBody()}
                    </div>
                  {:else}
                    <label
                      data-row-target
                      for={`tmodal-${t.id}`}
                      class={rowCoverClass(disabled ? "locked" : "toggle")}
                    >
                      {@render coverBody()}
                    </label>
                  {/if}
                </li>
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
                    {#if native !== null}
                      <!-- dir=auto: RTL/script autonyms must render in their own direction -->
                      (<span dir="auto">{native}</span>)
                    {/if}
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
          {/snippet}
          <nav
            data-language-rail
            aria-label={copy.translations.languagesLabel}
            class="{paneVisible ? 'hidden' : 'flex'} md:flex min-h-0 flex-col gap-0.5 overflow-y-auto overscroll-contain border-border p-2 md:border-e"
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
                class="flex h-[52px] flex-none cursor-pointer touch-manipulation items-center gap-2.5 rounded-lg px-2.5 text-start transition-colors {active
                  ? 'bg-primary/10 text-foreground'
                  : 'text-foreground-secondary hover:bg-surface-hover hover:text-foreground'}"
              >
                <span class="flex-none text-lg leading-none" aria-hidden="true">{l.flag}</span>
                <span class="flex min-w-0 flex-1 flex-col justify-center gap-0.5">
                  <span
                    data-language-name
                    class="truncate text-[15px] leading-tight {active ? 'font-medium' : ''}"
                  >
                    {l.language}
                  </span>
                  {#if l.autonym !== null}
                    <!-- dir=auto: RTL/script autonyms render in their own direction -->
                    <span
                      data-language-autonym
                      dir="auto"
                      class="truncate text-xs leading-tight text-muted-foreground"
                    >
                      {l.autonym}
                    </span>
                  {/if}
                </span>
                <span class="flex-none text-xs tabular-nums text-muted-foreground">
                  {l.entries.length}
                </span>
              </button>
            {/each}
          </nav>

          <section
            data-language-pane
            class="{paneVisible ? 'flex' : 'hidden'} md:flex min-h-0 flex-1 touch-manipulation flex-col overflow-y-auto overscroll-contain"
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
                  class="flex h-11 -ms-2 flex-none cursor-pointer touch-manipulation items-center gap-1 rounded-lg px-2 text-sm text-muted-foreground transition-colors hover:text-foreground md:hidden"
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
                  class="flex h-11 -ms-2 flex-none cursor-pointer touch-manipulation items-center gap-1 rounded-lg px-2 text-sm text-muted-foreground transition-colors hover:text-foreground md:hidden"
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
          class="flex h-11 flex-none cursor-pointer touch-manipulation items-center rounded-lg bg-primary px-5 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary-hover"
        >
          {copy.translations.done}
        </button>
      </div>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
