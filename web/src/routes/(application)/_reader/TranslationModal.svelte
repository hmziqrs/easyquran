<script lang="ts">
  import { Dialog } from "bits-ui";
  import { onMount } from "svelte";
  import { page } from "$app/state";
  import { replaceState } from "$app/navigation";
  import { translationSegmentsFromId } from "#lib/data/quran.js";
  import { STACKED_MAX_EXTRAS } from "#lib/data/quran-types.js";
  import type { TranslationCatalogueEntry } from "#lib/data/quran-types.js";
  import { withMoreParam } from "#lib/reader/more-param.js";
  import { visibleUrl } from "#lib/reader/visible-url.js";
  import {
    TRANSLATION_CATALOGUE,
    TRANSLATION_CATALOGUE_BY_ID,
    flagFor,
    nativeNameFor,
    translationSourceOf,
  } from "#lib/quran/catalogue.js";
  import type { TranslationProvenance } from "#lib/quran/catalogue.js";
  import { stackedTranslations } from "#lib/stores/stacked-translations.svelte.js";
  import { getReaderUiCopy } from "#lib/i18n/reader-copy.js";
  import { readerHrefFor } from "#lib/i18n/reader.js";
  import { publicHref } from "#lib/i18n/public-href.js";
  import { Icon } from "#lib/components/icon/index.js";
  import { hrefFor, liveReaderPosition } from "./translation-nav";
  import type { ReadPick } from "./reading-flow";
  import { translationMatchesQuery } from "./translation-search";

  type LanguageGroup = {
    language: string;
    code: string;
    flag: string;
    autonym: string | null;
    entries: TranslationCatalogueEntry[];
  };

  // Provenance rides every row's meta line as the source's own short name (a
  // proper noun, not copy); the full credit stays in the title attribute.
  const SOURCE_SHORT = {
    qul: "QUL",
    quranenc: "QuranEnc",
    tanzil: "Tanzil",
  } satisfies Record<TranslationProvenance, string>;

  let {
    open = $bindable(false),
    readPick = null,
  }: {
    open?: boolean;
    /** Reading's picker: one tap picks the text Reading flows instead of toggling the stack. */
    readPick?: ReadPick | null;
  } = $props();

  const picking = $derived(readPick !== null);
  const quickRows = $derived(
    (readPick?.quick ?? []).flatMap((id) => {
      const entry = TRANSLATION_CATALOGUE_BY_ID.get(id);
      return entry ? [entry] : [];
    }),
  );

  function pick(id: string): void {
    readPick?.onPick(id);
    open = false;
  }

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

  // Auto-select the rail language on open, where the reader most likely wants
  // to be: Reading anchors on the text it flows; the stack modal on its first
  // selected translation; with nothing selected, the reader's own top browser
  // language, then English, then the rail's first language.
  $effect(() => {
    if (!open || railLanguage !== null) return;
    const anchorId = readPick?.current ?? selectedIds[0] ?? null;
    const anchorEntry = anchorId !== null ? TRANSLATION_CATALOGUE_BY_ID.get(anchorId) : undefined;
    railLanguage = anchorEntry?.language ?? defaultLanguage();
  });

  function defaultLanguage(): string | null {
    for (const code of [...browserBoostCodes, "en"]) {
      const group = languages.find((l) => l.code === code);
      if (group) return group.language;
    }
    return languages[0]?.language ?? null;
  }

  // Live-url first (stress S1): on a scrolled surah route page.url still
  // holds the bare surah slug while window.location carries the reader's
  // /page/N rewrite (SvelteKit 2.70.2 replaceState never updates the page
  // store url). `open` re-keys the derivation on every open — window.location
  // is not reactive, and the reader can scroll (rewriting the live url) while
  // the modal is closed; a dedicated openedTick counter would read+write the
  // same state inside one effect and loop (effect_update_depth_exceeded).
  // While open, scroll lock keeps the live url frozen. null while closed is
  // inert: the dialog content is unmounted then.
  const position = $derived.by(() => {
    if (!open) return null;
    return liveReaderPosition(page.url);
  });
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
        // Stress S2: the native-script autonym is a search haystack, not just
        // a rail display line — "اردو" must find the Urdu rows.
        autonym: nativeNameFor(t.languageCode),
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

  // Keep the selected rail row in view: on open the auto-selected language can
  // sit far below the fold, and search filtering can shrink the rail around it.
  // "nearest" never scrolls when the row is already visible.
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

  // Selected chips: the stacked texts in store order. Updates live from store
  // changes; the row is absent when empty.
  const selectedRows = $derived.by(() => {
    const rows: TranslationCatalogueEntry[] = [];
    for (const id of selectedIds) {
      const found = TRANSLATION_CATALOGUE_BY_ID.get(id);
      if (found) rows.push(found);
    }
    return rows;
  });

  // Rail dot: languages holding a selected text (or Reading's flowing text).
  const languagesInUse = $derived.by(() => {
    const ids = picking ? [readPick?.current] : selectedIds;
    const inUse = new Set<string>();
    for (const id of ids) {
      const entry = id ? TRANSLATION_CATALOGUE_BY_ID.get(id) : undefined;
      if (entry) inUse.add(entry.language);
    }
    return inUse;
  });

  // Mobile: the pane replaces the rail once a language is chosen; searching
  // always shows the pane (the rail is filtered anyway). md+ shows both.
  const paneVisible = $derived(mobilePane || searchActive);

  // Selection changes only touch the stacked store + the ?more= URL param; the
  // reader layout's effects keep worker pinning in sync from there. The URL is
  // read via visibleUrl: kit 3 shallow writes never land in page.url, and a
  // stale page.url here would drop params that only exist in the visible URL.
  function syncUrl(): void {
    replaceState(withMoreParam(visibleUrl(page), stackedTranslations.ids), page.state);
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

  // Row cover classes (stress S9): one >=44px (min-h-11) cover per row. "toggle"
  // rows are labels whose whole surface — checkbox, name, author — toggles the
  // checkbox and keeps the modal open; "locked" mirrors a disabled checkbox
  // (cap reached). Navigating to a translation's own route is a separate
  // trailing control, never the name: a tap on a name must check it, not
  // navigate away and close.
  // touch-manipulation kills the double-tap-zoom window without breaking the
  // pane's scroll (stress S13).
  function rowCoverClass(kind: "toggle" | "locked"): string {
    const base = "flex min-h-11 min-w-0 flex-1 touch-manipulation items-center gap-3 px-3 py-2";
    if (kind === "toggle") return `${base} cursor-pointer`;
    return `${base} cursor-not-allowed opacity-50`;
  }
  function rowHref(t: TranslationCatalogueEntry): `/${string}` | null {
    const seg = translationSegmentsFromId(t.id);
    return hrefFor(position, { id: t.id, lang: seg.lang, translator: seg.translator });
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
    <Dialog.Overlay
      class="fixed inset-0 z-50 bg-black/50 data-[state=open]:animate-in data-[state=open]:fade-in-0"
    />
    <!-- Phones get a bottom sheet, md+ a centered dialog. Both have a FIXED height, so
         switching languages (17 rows vs 1) or searching never makes the surface jump. -->
    <Dialog.Content
      class="fixed bottom-0 left-0 right-0 z-50 flex h-[88dvh] w-full flex-col overflow-hidden rounded-t-2xl border-t border-border bg-popover bg-clip-padding pb-[env(safe-area-inset-bottom)] ps-[env(safe-area-inset-left)] pe-[env(safe-area-inset-right)] text-popover-foreground shadow-lg data-[state=open]:animate-in data-[state=open]:fade-in-0 max-md:data-[state=open]:slide-in-from-bottom-10 md:bottom-auto md:left-1/2 md:right-auto md:top-1/2 md:h-[min(720px,86vh)] md:w-[min(94vw,880px)] md:-translate-x-1/2 md:-translate-y-1/2 md:rounded-xl md:border md:pt-[env(safe-area-inset-top)] md:data-[state=open]:zoom-in-95"
    >
      <!-- S11 (stress A3): env(safe-area-inset-*) padding keeps modal content
           clear of notches/home indicators on devices that report insets. -->
      <div
        class="mx-auto mt-2 h-1 w-10 flex-none rounded-full bg-border-strong md:hidden"
        aria-hidden="true"
      ></div>

      <div class="flex flex-none flex-col gap-3 px-4 pb-3 pt-1 md:px-5 md:pt-3">
        <div class="flex items-center gap-3">
          <Dialog.Title class="text-[17px] font-semibold leading-tight">
            {picking ? copy.shell.readingTranslationPick : copy.stacked.title}
          </Dialog.Title>
          <Dialog.Description class="sr-only">{copy.translations.description}</Dialog.Description>
          <Dialog.Close>
            {#snippet child({ props })}
              <button
                {...props}
                type="button"
                aria-label={copy.translations.close}
                class="-me-2.5 ms-auto flex h-11 w-11 flex-none cursor-pointer touch-manipulation items-center justify-center rounded-lg text-muted-foreground transition-colors hover:bg-surface-hover hover:text-foreground"
              >
                <Icon name="x" size={16} />
              </button>
            {/snippet}
          </Dialog.Close>
        </div>

        <div
          class="flex h-11 items-center gap-2.5 rounded-lg border border-border bg-background-subtle px-3.5 transition-colors focus-within:border-border-strong md:h-10"
        >
          <Icon name="search" size={15} class="flex-none text-muted-foreground" />
          <input
            type="search"
            value={searchQuery}
            oninput={(e) => (searchQuery = e.currentTarget.value)}
            placeholder={copy.stacked.searchPlaceholder}
            aria-label={copy.stacked.searchPlaceholder}
            class="h-auto flex-1 border-0 bg-transparent px-0 py-0 text-base text-foreground shadow-none outline-none placeholder:text-muted-foreground focus-visible:outline-none md:text-sm [&::-webkit-search-cancel-button]:hidden"
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

        {#if picking}
          {#if quickRows.length > 0}
            <div data-quick-picks class="flex items-center gap-3">
              <span class="flex-none text-xs text-muted-foreground">{copy.shell.readingRecent}</span>
              <div
                class="flex min-w-0 flex-1 touch-manipulation items-center gap-1.5 overflow-x-auto overscroll-contain [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
              >
                {#each quickRows as t (t.id)}
                  {@const current = t.id === readPick?.current}
                  <button
                    type="button"
                    data-quick-pick={t.id}
                    aria-pressed={current}
                    onclick={() => pick(t.id)}
                    class="inline-flex h-11 max-w-full flex-none cursor-pointer touch-manipulation items-center gap-1.5 rounded-pill border px-3 text-[13px] font-medium transition-colors md:h-8 {current
                      ? 'border-primary bg-primary/10 text-foreground'
                      : 'border-border bg-background-subtle text-foreground hover:border-border-strong'}"
                  >
                    <span class="flex-none text-xs leading-none" aria-hidden="true"
                      >{flagFor(t.languageCode).flag}</span
                    >
                    <span class="min-w-0 truncate" dir="auto">{t.name}</span>
                  </button>
                {/each}
              </div>
            </div>
          {/if}
        {:else if selectedRows.length > 0}
          <div data-selected-chips class="flex items-center justify-between gap-3">
            <!-- S14 (stress B1): a single horizontal scroll row, never a wrapped
                 chip wall — the strip costs one row max and keeps touch
                 momentum; overscroll-contain stops end-of-strip flicks from
                 chaining to the page behind the modal (S12). -->
            <div
              class="flex min-w-0 flex-1 touch-manipulation items-center gap-1.5 overflow-x-auto overscroll-contain [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
            >
              {#each selectedRows as t (t.id)}
                {@const extraIndex = selectedIds.indexOf(t.id)}
                <div
                  data-chip={t.id}
                  class="group/chip inline-flex h-11 max-w-full flex-none touch-manipulation items-center gap-1 rounded-pill border border-border bg-background-subtle ps-2.5 pe-0.5 transition-colors hover:border-border-strong md:h-8"
                >
                  <span class="flex-none text-xs leading-none" aria-hidden="true">
                    {flagFor(t.languageCode).flag}
                  </span>
                  <span class="min-w-0 truncate text-[13px] font-medium text-foreground">
                    <bdi>{t.name}</bdi>
                  </span>
                  <!-- S8/S9 (stress A2/A1): the hover-reveal compiles only
                       under @media(hover:hover), so coarse pointers get the
                       arrows always (via the hover:none variant), keyboard
                       focus on the chip reveals them too, and the
                       before-pseudo widens each 44px-tall control to a 44px
                       hit area without growing the chip. Hidden arrows also
                       collapse to w-0 on hover devices — opacity alone
                       still reserved 28px per button, leaving ragged holes
                       inside the pill (idle chips read name…✕ tight);
                       overflow-hidden there is safe because the before-pseudo
                       hit extension only matters on touch, where w-7 stays. -->
                  <button
                    type="button"
                    onclick={() => reorder(t.id, -1)}
                    disabled={extraIndex <= 0}
                    aria-label={copy.stacked.moveUp}
                    class="relative flex h-11 w-7 flex-none cursor-pointer touch-manipulation items-center justify-center rounded-md text-muted-foreground opacity-0 transition-[width,opacity] before:absolute before:-inset-x-2 before:inset-y-0 before:content-[''] hover:text-foreground focus-visible:opacity-100 group-hover/chip:opacity-100 group-focus-within/chip:opacity-100 md:h-8 [@media(hover:hover)]:w-0 [@media(hover:hover)]:overflow-hidden [@media(hover:hover)]:group-hover/chip:w-7 [@media(hover:hover)]:group-focus-within/chip:w-7 [@media(hover:hover)]:focus-visible:w-7 [@media(hover:none)]:enabled:opacity-100 disabled:cursor-not-allowed disabled:opacity-30"
                  >
                    <Icon name="arrow-right" size={12} class="-rotate-90" />
                  </button>
                  <button
                    type="button"
                    onclick={() => reorder(t.id, 1)}
                    disabled={extraIndex === -1 || extraIndex >= selectedIds.length - 1}
                    aria-label={copy.stacked.moveDown}
                    class="relative flex h-11 w-7 flex-none cursor-pointer touch-manipulation items-center justify-center rounded-md text-muted-foreground opacity-0 transition-[width,opacity] before:absolute before:-inset-x-2 before:inset-y-0 before:content-[''] hover:text-foreground focus-visible:opacity-100 group-hover/chip:opacity-100 group-focus-within/chip:opacity-100 md:h-8 [@media(hover:hover)]:w-0 [@media(hover:hover)]:overflow-hidden [@media(hover:hover)]:group-hover/chip:w-7 [@media(hover:hover)]:group-focus-within/chip:w-7 [@media(hover:hover)]:focus-visible:w-7 [@media(hover:none)]:enabled:opacity-100 disabled:cursor-not-allowed disabled:opacity-30"
                  >
                    <Icon name="arrow-right" size={12} class="rotate-90" />
                  </button>
                  <button
                    type="button"
                    onclick={() => remove(t.id)}
                    aria-label={copy.stacked.remove}
                    class="relative flex h-11 w-7 flex-none cursor-pointer touch-manipulation items-center justify-center rounded-md text-muted-foreground transition-colors before:absolute before:-inset-x-2 before:inset-y-0 before:content-[''] hover:text-foreground md:h-8"
                  >
                    <Icon name="x" size={13} />
                  </button>
                </div>
              {/each}
            </div>
            <div
              class="flex flex-none touch-manipulation items-center gap-2.5 text-xs text-muted-foreground"
            >
              <span class="tabular-nums" title={copy.translations.capNote(STACKED_MAX_EXTRAS)}>
                {copy.stacked.count(selectedIds.length, STACKED_MAX_EXTRAS)}
              </span>
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
            </div>
          </div>
        {/if}
      </div>

      <div
        class="grid min-h-0 flex-1 overflow-hidden border-t border-border md:grid-cols-[232px_minmax(0,1fr)]"
      >
        {#snippet translationRow(t: TranslationCatalogueEntry, withLanguage: boolean)}
          {@const checked = picking ? t.id === readPick?.current : selectedIds.includes(t.id)}
          {@const disabled = !picking && isFull && !checked}
          {@const href = picking ? null : rowHref(t)}
          {@const source = translationSourceOf(t.id)}
          <!-- One row anatomy everywhere: a fixed two-line block (name, then a muted
               meta line — language when searching, translator when it differs from
               the name, and always the source), so every row has the same height and
               the list reads as one rhythm. The row body is a <label> tied to the row
               checkbox (S9/S10), so the whole surface is one >=44px toggle target;
               the go-to link is a separate trailing control beside it. -->
          <li
            data-translation-row={t.id}
            class="group/row flex rounded-lg transition-colors {checked
              ? 'bg-primary/10'
              : 'hover:bg-surface-hover'}"
          >
            {#snippet coverBody()}
              {#if picking}
                <!-- Reading's picker: a check marks the text flowing now, no checkbox. -->
                <span
                  class="flex size-[18px] flex-none items-center justify-center text-primary"
                  aria-hidden="true"
                >
                  {#if checked}<Icon name="check" size={16} />{/if}
                </span>
              {:else}
                <!-- Native checkbox, restyled: appearance-none keeps every native
                     behaviour (label toggle, keyboard, disabled) while the box and
                     its check read in the theme instead of the browser grey. -->
                <span class="relative flex size-[18px] flex-none items-center justify-center">
                  <input
                    id={`tmodal-${t.id}`}
                    type="checkbox"
                    {checked}
                    {disabled}
                    onchange={() => toggle(t.id)}
                    aria-label={rowLabel(t)}
                    class="peer size-[18px] flex-none cursor-pointer touch-manipulation appearance-none rounded-[5px] border-[1.5px] border-border-strong bg-transparent transition-colors checked:border-primary checked:bg-primary focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring disabled:cursor-not-allowed"
                  />
                  <Icon
                    name="check"
                    size={12}
                    class="pointer-events-none absolute text-primary-foreground opacity-0 peer-checked:opacity-100"
                  />
                </span>
              {/if}
              <span class="flex min-w-0 flex-1 flex-col gap-0.5">
                <span class="truncate text-[14px] font-medium leading-5 text-foreground">
                  <bdi>{t.name}</bdi>
                </span>
                <span
                  class="flex min-w-0 items-center gap-1.5 text-[12px] leading-4 text-muted-foreground"
                >
                  {#if withLanguage}
                    <span data-row-language class="flex-none">
                      <span aria-hidden="true">{flagFor(t.languageCode).flag}</span>
                      {t.language}
                    </span>
                    <span class="flex-none" aria-hidden="true">·</span>
                  {/if}
                  {#if hasAuthorLine(t)}
                    <span data-author-line class="min-w-0 truncate"><bdi>{t.translator}</bdi></span>
                    <span class="flex-none" aria-hidden="true">·</span>
                  {/if}
                  <span
                    data-row-source
                    class="flex-none"
                    title={copy.translations.sourceLabel(source)}>{SOURCE_SHORT[source]}</span
                  >
                </span>
              </span>
            {/snippet}
            {#if picking}
              <button
                type="button"
                data-row-pick={t.id}
                aria-pressed={checked}
                onclick={() => pick(t.id)}
                class="{rowCoverClass('toggle')} text-start"
              >
                {@render coverBody()}
              </button>
            {:else}
              <label
                data-row-target
                for={`tmodal-${t.id}`}
                class={rowCoverClass(disabled ? "locked" : "toggle")}
              >
                {@render coverBody()}
              </label>
            {/if}
            {#if href}
              <!-- Navigate to this translation's own route at the same
                   reading position — pure navigation, no side effects.
                   Revealed on row hover/focus where a pointer can
                   hover; always shown on touch. -->
              <a
                href={publicHref(readerHrefFor(copy.locale, href))}
                data-switch
                data-sveltekit-preload-data="hover"
                onclick={() => (open = false)}
                aria-label={`${copy.translations.goTo}: ${rowLabel(t)}`}
                title={copy.translations.goTo}
                class="flex w-11 flex-none cursor-pointer touch-manipulation items-center justify-center self-stretch rounded-lg text-muted-foreground transition-[opacity,color] hover:text-foreground focus-visible:opacity-100 [@media(hover:hover)]:opacity-0 [@media(hover:hover)]:group-hover/row:opacity-100 [@media(hover:hover)]:group-focus-within/row:opacity-100"
              >
                <Icon name="arrow-right" size={15} />
              </a>
            {/if}
          </li>
        {/snippet}

        <nav
          data-language-rail
          aria-label={copy.translations.languagesLabel}
          class="{paneVisible
            ? 'hidden'
            : 'flex'} min-h-0 flex-col gap-0.5 overflow-y-auto overscroll-contain border-border p-2 md:flex md:border-e"
        >
          {#each languages as l, i (l.language)}
            {@const active = l.language === activeLanguage}
            <!-- One line per language: flag, name, autonym (muted, truncating),
                 an in-use dot when one of its translations is selected, count. -->
            <button
              data-language-option={l.language}
              type="button"
              tabindex={active ? 0 : -1}
              aria-current={active ? "true" : undefined}
              onclick={() => selectLanguage(l.language)}
              onkeydown={(e) => onRailKeydown(e, i)}
              class="flex h-11 flex-none cursor-pointer touch-manipulation items-center gap-2.5 rounded-md px-2.5 text-start transition-colors md:h-9 {active
                ? 'bg-surface-hover text-foreground'
                : 'text-foreground-secondary hover:bg-surface-hover hover:text-foreground'}"
            >
              <span class="flex-none text-base leading-none" aria-hidden="true">{l.flag}</span>
              <span class="flex min-w-0 flex-1 items-baseline gap-1.5">
                <span
                  data-language-name
                  class="flex-none text-[14px] {active ? 'font-semibold' : 'font-medium'}"
                  >{l.language}</span
                >
                {#if l.autonym !== null}
                  <!-- bdi dir=auto: an RTL autonym keeps its own direction but
                       stays start-aligned after the name. -->
                  <bdi
                    data-language-autonym
                    dir="auto"
                    class="min-w-0 truncate text-[12px] text-muted-foreground">{l.autonym}</bdi
                  >
                {/if}
              </span>
              {#if languagesInUse.has(l.language)}
                <span
                  data-language-in-use
                  class="size-1.5 flex-none rounded-full bg-primary"
                  aria-hidden="true"
                ></span>
              {/if}
              <span class="flex-none text-[12px] tabular-nums text-muted-foreground">
                {l.entries.length}
              </span>
            </button>
          {/each}
        </nav>

        <section
          data-language-pane
          class="{paneVisible
            ? 'flex'
            : 'hidden'} min-h-0 flex-1 touch-manipulation flex-col overflow-x-hidden overflow-y-auto overscroll-contain md:flex"
        >
          {#snippet paneBack()}
            <button
              type="button"
              onclick={backToRail}
              aria-label={copy.translations.back}
              class="-ms-2 flex h-11 flex-none cursor-pointer touch-manipulation items-center gap-1 rounded-lg px-2 text-sm text-muted-foreground transition-colors hover:text-foreground md:hidden"
            >
              <Icon name="arrow-right" size={14} class="rotate-180" />
              {copy.translations.back}
            </button>
          {/snippet}
          <!-- Sticky pane header: the language stays named while its list scrolls. -->
          {#if searchActive}
            <div
              class="sticky top-0 z-10 flex h-12 flex-none items-center gap-2 border-b border-border bg-popover px-4"
            >
              {@render paneBack()}
              <p data-results-count class="text-[13px] text-muted-foreground">
                {copy.translations.results(matches.length)}
              </p>
            </div>
            {#if matches.length === 0}
              <p class="px-4 py-6 text-sm text-muted-foreground" role="status">
                {copy.translations.noMatches}
              </p>
            {:else}
              <ul class="flex flex-col gap-0.5 p-2">
                {#each searchRows as t (t.id)}
                  {@render translationRow(t, true)}
                {/each}
              </ul>
            {/if}
          {:else if activeLanguage !== null}
            <div
              class="sticky top-0 z-10 flex h-12 flex-none items-center gap-2 border-b border-border bg-popover px-4"
            >
              {@render paneBack()}
              <h3 class="text-[15px] font-semibold text-foreground">{activeLanguage}</h3>
              <span data-results-count class="text-[13px] text-muted-foreground">
                {copy.translations.results(activeEntries.length)}
              </span>
            </div>
            <ul class="flex flex-col gap-0.5 p-2">
              {#each activeEntries as t (t.id)}
                {@render translationRow(t, false)}
              {/each}
            </ul>
          {/if}
        </section>
      </div>

      <div
        class="flex flex-none items-center justify-between gap-3 border-t border-border px-4 py-3 md:px-5"
      >
        <p data-cap-note class="text-xs text-muted-foreground">
          {#if picking}
            {copy.shell.readingPickNote}
          {:else if isFull}
            {copy.stacked.full(STACKED_MAX_EXTRAS)}
          {:else}
            {copy.translations.capNote(STACKED_MAX_EXTRAS)}
          {/if}
        </p>
        <button
          type="button"
          data-done
          onclick={() => (open = false)}
          class="flex h-11 flex-none cursor-pointer touch-manipulation items-center rounded-lg bg-primary px-5 text-sm font-semibold text-primary-foreground transition-colors hover:bg-primary-hover md:h-9"
        >
          {copy.translations.done}
        </button>
      </div>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
