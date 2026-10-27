<script lang="ts">
  import { Dialog } from "bits-ui";
  import { onMount } from "svelte";
  import { SvelteSet } from "svelte/reactivity";
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
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
  import {
    groupTranslationAuthors,
    translationAuthorKey,
    translationProvenanceRank,
  } from "#lib/quran/translation-groups.js";
  import { isTafsirTranslation } from "#lib/quran/text-marks.js";
  import type { TranslationProvenance } from "#lib/quran/catalogue.js";
  import { stackedTranslations } from "#lib/stores/stacked-translations.svelte.js";
  import { storageReport } from "#lib/stores/storage-report.svelte.js";
  import { translationRecents } from "#lib/stores/translation-recents.svelte.js";
  import { getReaderUiCopy } from "#lib/i18n/reader-copy.js";
  import {
    reader_tooltip_offline,
    reader_tooltip_offline_downloaded,
    reader_tooltip_offline_needs_internet,
    reader_translations_commentary,
    reader_translations_recent,
  } from "#lib/i18n/m/reader.js";
  import { readerHrefFor } from "#lib/i18n/reader.js";
  import { publicHref } from "#lib/i18n/public-href.js";
  import { Icon } from "#lib/components/icon/index.js";
  import {
    Tooltip,
    TooltipContent,
    TooltipProvider,
    TooltipTrigger,
  } from "#lib/components/ui/tooltip/index.js";
  import { hrefFor, liveReaderPosition } from "#lib/reader/translation-nav.js";
  import type { ReadPick } from "#lib/reader/reading-flow.js";
  import { translationMatchesQuery } from "./translation-search";
  import { detectCountries, languagePriority } from "./translation-region";

  type LanguageGroup = {
    language: string;
    code: string;
    flag: string;
    autonym: string | null;
    entries: TranslationCatalogueEntry[];
  };

  // One author, multiple sources (Feature 8): one row per author; a
  // multi-source author collapses onto its representative variant.
  type AuthorGroup = {
    key: string;
    representative: TranslationCatalogueEntry;
    rest: readonly TranslationCatalogueEntry[];
  };

  // Provenance rides every row's meta line as the source's own short name (a
  // proper noun, not copy); the full credit stays in the title attribute.
  const SOURCE_SHORT = {
    qul: "QUL",
    quranenc: "QuranEnc",
    tanzil: "Tanzil",
  } satisfies Record<TranslationProvenance, string>;
  // Provenance colour identity lives only inside the row's metadata tooltip:
  // the dot reads on the inverted (bg-foreground) surface in both modes.
  const PROVENANCE_DOT = {
    qul: "bg-violet-500",
    quranenc: "bg-sky-500",
    tanzil: "bg-emerald-500",
  } satisfies Record<TranslationProvenance, string>;

  let {
    open = $bindable(false),
    readPick = null,
    onStackChanged = null,
  }: {
    open?: boolean;
    /** Reading's picker: one tap picks the text Reading flows instead of toggling the stack. */
    readPick?: ReadPick | null;
    /** Fired once when the dialog closes after ≥1 user-driven stack mutation
     * (add/remove/reorder/clear/pause). Deferred to close so hosts' follow-ups
     * — the azkar page's card offer toast — never land behind the open modal;
     * and initial hydration from localStorage never reads as a user change. */
    onStackChanged?: (() => void) | null;
  } = $props();

  let stackDirty = false;

  const picking = $derived(readPick !== null);
  // Tafsir works are commentary, not translations (TRX-01): they are offered
  // ONLY under the Commentary section below — never as quick picks, recents,
  // search results, or in a language's main list, and never as a reading pick.
  const quickRows = $derived(
    (readPick?.quick ?? []).flatMap((id) => {
      const entry = TRANSLATION_CATALOGUE_BY_ID.get(id);
      if (!entry || isTafsirTranslation(entry.id)) return [];
      return [entry];
    }),
  );

  /** The pinned Recent group holds at most this many of the MRU's entries. */
  const RECENT_GROUP_MAX = 5;
  // Global MRU rows (empty for an untouched reader — no group at all). Recents
  // rank the AUTHOR, not the variant: later variants of a seen author are skipped.
  const recentRows = $derived.by(() => {
    const seen = new Set<string>();
    const rows: TranslationCatalogueEntry[] = [];
    for (const id of translationRecents.ids) {
      if (rows.length >= RECENT_GROUP_MAX) break;
      const entry = TRANSLATION_CATALOGUE_BY_ID.get(id);
      if (!entry || isTafsirTranslation(entry.id)) continue;
      const key = translationAuthorKey(entry);
      if (seen.has(key)) continue;
      seen.add(key);
      rows.push(entry);
    }
    return rows;
  });

  // The Commentary section's rows: the frozen tafsir allow-list from the
  // catalogue, shown under their own heading inside their language's pane.
  const commentaryRows = $derived(TRANSLATION_CATALOGUE.filter((t) => isTafsirTranslation(t.id)));

  // Every explicit selection records the MRU head and the author's last-used
  // variant (Feature 1.4), which survives the MRU's 20-entry cap.
  function recordUse(id: string): void {
    const entry = TRANSLATION_CATALOGUE_BY_ID.get(id);
    translationRecents.record(
      id,
      Date.now(),
      entry === undefined ? undefined : translationAuthorKey(entry),
    );
  }

  function pick(id: string): void {
    // An explicit picker choice is a use; record before the pick flows so
    // orderings derived from the MRU (Reading's chips) see the new head at once.
    recordUse(id);
    readPick?.onPick(id);
    open = false;
  }

  const copy = getReaderUiCopy();
  // Locale changes navigate (full reload), so resolving once matches getReaderUiCopy's lifecycle.
  const recentLabel = reader_translations_recent(undefined, { locale: copy.locale });
  const commentaryLabel = reader_translations_commentary(undefined, { locale: copy.locale });
  // TRX-16 hover-card availability line, resolved the same one-shot way.
  const offlineLabel = reader_tooltip_offline(undefined, { locale: copy.locale });
  const downloadedLabel = reader_tooltip_offline_downloaded(undefined, { locale: copy.locale });
  const needsInternetLabel = reader_tooltip_offline_needs_internet(undefined, {
    locale: copy.locale,
  });
  let searchQuery = $state("");
  // Selected rail language, held as its CODE; null until the modal first opens.
  // Codes are the rail's identity because display names are not unique — the
  // catalogue renders "Amazigh" for both ber and zgh — so name-keyed selection
  // could never tell those tabs (or their panes) apart.
  let railCode = $state<string | null>(null);
  // Mobile only: true once a language is tapped, showing its pane over the rail.
  let mobilePane = $state(false);
  // Author groups whose source variants are expanded. SvelteSet: a plain Set
  // in $state does not observe has/add in this build.
  let expandedAuthors = new SvelteSet<string>();

  // Reset transient navigation state whenever the modal closes.
  $effect(() => {
    if (!open) {
      searchQuery = "";
      railCode = null;
      mobilePane = false;
      expandedAuthors.clear();
    }
  });

  // Hosts learn about stack changes on close, once per editing session, so
  // follow-up offers (azkar's card translation) are clickable and un-duplicated.
  $effect(() => {
    if (open || !stackDirty) return;
    stackDirty = false;
    onStackChanged?.();
  });

  // Client-only reader signals for the rail's priority sort (see
  // priorityCodes below). onMount never runs on the server, so the SSR render
  // keeps the signal-free order and hydration cannot mismatch.
  onMount(() => {
    priorityCodes = languagePriority(navigator.languages, detectCountries());
  });

  // Auto-select the rail language on open, where the reader most likely wants
  // to be: Reading anchors on the text it flows; the stack modal on its first
  // selected translation; with nothing selected, the reader's own top browser
  // language, then English, then the rail's first language.
  $effect(() => {
    if (!open || railCode !== null) return;
    const anchorId = readPick?.current ?? selectedIds[0] ?? null;
    const anchorEntry = anchorId !== null ? TRANSLATION_CATALOGUE_BY_ID.get(anchorId) : undefined;
    railCode = anchorEntry?.languageCode ?? defaultLanguage();
  });

  function defaultLanguage(): string | null {
    for (const code of priorityCodes) {
      if (languages.some((l) => l.code === code)) return code;
    }
    return languages[0]?.code ?? null;
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

  // Search never offers tafsir (TRX-01) — commentary is reached through its
  // own section, so its ids are filtered out of the match pool entirely.
  const matches = $derived.by(() => {
    const q = searchQuery.trim();
    if (!q) return TRANSLATION_CATALOGUE;
    return TRANSLATION_CATALOGUE.filter(
      (t) =>
        !isTafsirTranslation(t.id) &&
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
    // One rail tab per language CODE: the catalogue carries two display-name
    // variants for some codes (az, dv, ug, …), and grouping by the raw
    // language string split those into two tabs. The first entry's display
    // name labels the merged tab.
    const map = new Map<string, TranslationCatalogueEntry[]>();
    for (const t of rows) {
      const arr = map.get(t.languageCode);
      if (arr) arr.push(t);
      else map.set(t.languageCode, [t]);
    }
    return [...map.entries()]
      .map(([code, entries]) => {
        return {
          language: entries[0]?.language ?? "",
          code,
          flag: flagFor(code).flag,
          autonym: nativeNameFor(code),
          entries: [...entries].sort((a, b) => languageCollator.compare(a.name, b.name)),
        };
      })
      .sort((a, b) => languageCollator.compare(a.language, b.language));
  }

  // Rail priority as language codes (translation-region.ts): the reader's
  // first browser language, English, their other browser languages, the
  // languages of the country their timezone / language tags point to, then
  // Arabic. Starts signal-free (English, Arabic) so SSR and the first client
  // render agree; onMount swaps in the reader's own signals.
  let priorityCodes = $state.raw<string[]>(languagePriority([], []));

  // Codes not in the priority list rank after it, alphabetically.
  function railRank(group: LanguageGroup): number {
    const index = priorityCodes.indexOf(group.code);
    if (index !== -1) return index;
    return Number.MAX_SAFE_INTEGER;
  }
  // The suggested head of the rail, set off from the A–Z tail by a hairline.
  function isSuggested(group: LanguageGroup | undefined): boolean {
    return group !== undefined && priorityCodes.includes(group.code);
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

  // The active rail group, resolved by CODE (railCode, else the rail's head).
  // Name matching would be ambiguous: ber and zgh both display "Amazigh", and
  // a name match always lands on the first tab, leaving the other unreachable.
  const activeGroup = $derived.by(() => {
    if (railCode !== null) {
      const group = languages.find((l) => l.code === railCode);
      if (group) return group;
    }
    return languages[0];
  });

  // Keep the selected rail row in view: on open the auto-selected language can
  // sit far below the fold, and search filtering can shrink the rail around it.
  // "nearest" never scrolls when the row is already visible.
  $effect(() => {
    if (!open || activeGroup === undefined) return;
    const row = document.querySelector<HTMLElement>(
      `[data-language-option="${CSS.escape(activeGroup.language)}"]`,
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

  // Search results group by author too (one row per author per language), so
  // the same work from two sources never reads as indistinguishable rows.
  const searchGroups = $derived(
    groupTranslationAuthors(searchRows).map((g) => orderAuthorGroup(g.key, g.variants)),
  );

  // Sort key for the rail boost: never-used translations share one rank after
  // every recently used one, so the stable sort keeps the alphabetical order
  // below the boosted head.
  function recencyRank(id: string): number {
    const rank = translationRecents.rankOf(id);
    return rank === -1 ? Number.MAX_SAFE_INTEGER : rank;
  }

  // The active rail stays alphabetical until real usage exists; once the MRU
  // holds entries, its picks for this language float to the top. Tafsir works
  // never join the main list (TRX-01) — they render under the Commentary
  // section inside their own language's pane instead.
  const activeEntries = $derived.by(() => {
    const entries = activeGroup?.entries ?? [];
    return entries
      .filter((t) => !isTafsirTranslation(t.id))
      .sort((a, b) => recencyRank(a.id) - recencyRank(b.id));
  });

  // One row per author: the entries above are already recency-then-name
  // ordered, so first-seen group order keeps that ranking per author.
  const activeGroups = $derived(
    groupTranslationAuthors(activeEntries).map((g) => orderAuthorGroup(g.key, g.variants)),
  );

  // The variant a collapsed author row resolves to: the flowing/selected one,
  // else the author's most recently used, else the original publisher
  // (tanzil/quranenc ahead of QUL mirrors).
  function isSelected(id: string): boolean {
    if (picking) return id === readPick?.current;
    return selectedIds.includes(id);
  }
  function compareRepresentatives(
    key: string,
    a: TranslationCatalogueEntry,
    b: TranslationCatalogueEntry,
  ): number {
    const selectedDelta = Number(isSelected(b.id)) - Number(isSelected(a.id));
    if (selectedDelta !== 0) return selectedDelta;
    const recencyDelta = recencyRank(a.id) - recencyRank(b.id);
    if (recencyDelta !== 0) return recencyDelta;
    // Feature 1.4: the author's last-used variant outlives MRU eviction.
    const remembered = translationRecents.authorVariant(key);
    if (remembered !== undefined) {
      if (a.id === remembered) return -1;
      if (b.id === remembered) return 1;
    }
    return translationProvenanceRank(a.id) - translationProvenanceRank(b.id);
  }
  function orderAuthorGroup(
    key: string,
    variants: readonly TranslationCatalogueEntry[],
  ): AuthorGroup {
    const [head, ...tail] = variants;
    if (head === undefined) throw new Error(`[translations] empty author group ${key}`);
    if (tail.length === 0) return { key, representative: head, rest: [] };
    let best = 0;
    for (let i = 1; i < variants.length; i += 1) {
      if (compareRepresentatives(key, variants[i]!, variants[best]!) < 0) best = i;
    }
    return {
      key,
      representative: variants[best]!,
      rest: variants.filter((_, i) => i !== best),
    };
  }
  function toggleAuthorSources(key: string): void {
    if (expandedAuthors.has(key)) expandedAuthors.delete(key);
    else expandedAuthors.add(key);
  }

  // The Commentary section renders only inside the tafsir language's pane and
  // never in the reading picker (a tafsir is never the reading translation).
  const commentaryVisible = $derived.by(() => {
    if (picking || searchActive || activeGroup === undefined) return false;
    return commentaryRows.some((t) => t.language === activeGroup.language);
  });

  // The pane's count line covers every row it lists, Commentary included, so
  // the tafsir language's pane never reads "0 translations" over two rows.
  const paneRowCount = $derived(
    activeEntries.length + (commentaryVisible ? commentaryRows.length : 0),
  );

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

  // Rail dot: languages (by code, matching the rail's grouping) holding a
  // selected text (or Reading's flowing text).
  const languagesInUse = $derived.by(() => {
    const ids = picking ? [readPick?.current] : selectedIds;
    const inUse = new Set<string>();
    for (const id of ids) {
      const entry = id ? TRANSLATION_CATALOGUE_BY_ID.get(id) : undefined;
      if (entry) inUse.add(entry.languageCode);
    }
    return inUse;
  });

  // Mobile: the pane replaces the rail once a language is chosen; searching
  // always shows the pane (the rail is filtered anyway). md+ shows both.
  const paneVisible = $derived(mobilePane || searchActive);

  // TRX-16: the hover card's offline line derives from the verse-cache state
  // (the storage report's artifacts). Hydrated lazily on first open, so the
  // reader page never queries artifacts until the picker is opened.
  const cachedIds = $derived(new Set(storageReport.artifacts.map((artifact) => artifact.id)));
  $effect(() => {
    if (open) storageReport.hydrate();
  });

  // Selection changes only touch the stacked store + the ?more= URL param; the
  // reader layout's effects keep worker pinning in sync from there. The URL is
  // read via visibleUrl: kit 3 shallow writes never land in page.url, and a
  // stale page.url here would drop params that only exist in the visible URL.
  function syncUrl(): void {
    // Shallow replace-goto: kit 3's replacement for the deprecated replaceState.
    void goto(withMoreParam(visibleUrl(page), stackedTranslations.ids), {
      state: page.state,
      shallow: true,
      replace: true,
    });
  }
  function toggle(id: string): void {
    const wasSelected = selectedIds.includes(id);
    stackedTranslations.toggle(id);
    // Stacking a translation is an explicit choice; unstacking one is not.
    if (!wasSelected) {
      // An explicit add while the stack is paused resumes the flow — checking a
      // box and seeing nothing appear would read as breakage.
      stackedTranslations.setEnabled(true);
      recordUse(id);
    }
    syncUrl();
    stackDirty = true;
  }
  function reorder(id: string, delta: -1 | 1): void {
    stackedTranslations.reorder(id, delta);
    syncUrl();
    stackDirty = true;
  }
  function remove(id: string): void {
    stackedTranslations.remove(id);
    syncUrl();
    stackDirty = true;
  }
  function clear(): void {
    stackedTranslations.clear();
    syncUrl();
    stackDirty = true;
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
    // Commentary has no route of its own (TRX-01): a tafsir URL 308s back to
    // the Arabic reader, so its rows carry no go-to arrow at all.
    if (picking || isTafsirTranslation(t.id)) return null;
    const seg = translationSegmentsFromId(t.id);
    return hrefFor(position, { id: t.id, lang: seg.lang, translator: seg.translator });
  }

  function formatSize(bytes: number): string {
    if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${Math.max(1, Math.round(bytes / 1024))} KB`;
  }

  function selectLanguage(code: string): void {
    railCode = code;
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
    const group = languages[next];
    if (group === undefined) return;
    railCode = group.code;
    // All rail options are already in the DOM, so focus can move synchronously
    // without waiting for the selection state to re-render.
    document
      .querySelector<HTMLElement>(`[data-language-option="${CSS.escape(group.language)}"]`)
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
          <Dialog.Title class="text-[17px] font-medium leading-tight">
            {picking ? copy.shell.readingTranslationPick : copy.stacked.title}
          </Dialog.Title>
          <Dialog.Description class="sr-only">{copy.translations.description}</Dialog.Description>
          <Dialog.Close>
            {#snippet child({ props })}
              <!-- TRX-08: exactly one close control per surface — Done at the
                   bottom on phones, this ✕ on md+. -->
              <button
                {...props}
                type="button"
                aria-label={copy.translations.close}
                class="-me-2.5 ms-auto hidden h-11 w-11 flex-none cursor-pointer touch-manipulation items-center justify-center rounded-lg text-muted-foreground transition-colors hover:bg-surface-hover hover:text-foreground md:flex"
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
          <!-- Master switch: pause the whole stack without touching the selection.
               Same aria-pressed pill idiom as the settings privacy rows; the chips
               below stay live so the reader can curate while translations are off. -->
          <div data-stack-toggle class="flex items-center justify-between gap-3">
            <span class="text-[13px] font-medium text-foreground">{copy.stacked.show}</span>
            <button
              type="button"
              data-show-toggle
              aria-pressed={stackedTranslations.enabled}
              onclick={() => {
                stackedTranslations.toggleEnabled();
                stackDirty = true;
              }}
              class="flex h-11 flex-none cursor-pointer touch-manipulation items-center rounded-pill border px-3.5 text-caption transition-colors duration-150 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring md:h-8 {stackedTranslations.enabled
                ? 'border-primary bg-primary-soft text-foreground'
                : 'border-border-strong text-foreground-secondary hover:border-border hover:text-foreground'}"
            >
              {stackedTranslations.enabled ? copy.stacked.on : copy.stacked.off}
            </button>
          </div>
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

      <TooltipProvider delayDuration={400}>
      <div
        class="grid min-h-0 flex-1 grid-rows-[auto_minmax(0,1fr)] overflow-hidden border-t border-border md:grid-cols-[232px_minmax(0,1fr)]"
      >
        {#snippet translationRow(
          t: TranslationCatalogueEntry,
          withLanguage: boolean,
          list: "rail" | "recent" | "search" | "tafsir" | "variant",
          sources?: { key: string; count: number; expanded: boolean },
        )}
          {@const checked = picking ? t.id === readPick?.current : selectedIds.includes(t.id)}
          {@const disabled = !picking && isFull && !checked}
          {@const href = rowHref(t)}
          {@const source = translationSourceOf(t.id)}
          {@const native = nativeNameFor(t.languageCode)}
          <!-- The Recent group and the active rail can show the same translation
               at once, so each list namespaces its checkbox id — a duplicate id
               would make both labels resolve to the first copy. -->
          {@const domId = `tmodal-${list}-${t.id}`}
          <!-- One row anatomy everywhere: a fixed two-line block (name, then a muted
               meta line — language when searching, translator when it differs from
               the name, and always the source), so every row has the same height and
               the list reads as one rhythm. The row body is a <label> tied to the row
               checkbox (S9/S10), so the whole surface is one >=44px toggle target;
               the go-to link is a separate trailing control beside it. -->
          <!-- Metadata tooltip (translator, language, size, direction, source)
               triggers from the whole row on hover, and from keyboard focus on the
               row's checkbox/link via the bubbling focusin/focusout. The row itself
               is not a tab stop (tabindex -1). SAFETY: the child-snippet props bag
               is untyped, so the forwarded handlers carry a FocusEvent cast — they
               are bits-ui's own onfocus/onblur. -->
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
            class="group/row flex rounded-lg transition-colors {list === 'variant' ? 'ms-5' : ''} {checked
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
                    id={domId}
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
                  {#if list === "variant"}
                    <!-- A variant hangs off its author row above: its own size
                         + source are the differentiators. -->
                    <span data-row-size class="min-w-0 truncate">{formatSize(t.sizeBytes)}</span>
                    <span class="flex-none" aria-hidden="true">·</span>
                  {:else if hasAuthorLine(t)}
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
                for={domId}
                class={rowCoverClass(disabled ? "locked" : "toggle")}
              >
                {@render coverBody()}
              </label>
            {/if}
            {#if sources}
              <!-- "Show all sources" disclosure for a multi-source author; the
                   count names the variants it reveals. -->
              <button
                type="button"
                data-sources-toggle={t.id}
                aria-expanded={sources.expanded}
                onclick={() => toggleAuthorSources(sources.key)}
                class="flex h-11 flex-none cursor-pointer touch-manipulation items-center gap-1 self-stretch rounded-lg px-2 text-[12px] tabular-nums text-muted-foreground transition-colors hover:text-foreground md:h-auto"
              >
                {copy.translations.results(sources.count)}
                <Icon
                  name="arrow-right"
                  size={12}
                  class="flex-none transition-transform {sources.expanded
                    ? '-rotate-90'
                    : 'rotate-90'}"
                />
              </button>
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
            </TooltipTrigger>
            <TooltipContent
              class="flex w-[260px] max-w-[260px] flex-col items-start gap-1.5 whitespace-normal rounded-md px-3 py-2.5 text-start leading-snug"
            >
              <span class="text-[12px] font-medium"><bdi>{t.name}</bdi></span>
              <span
                class="inline-flex items-center gap-1.5 rounded-pill bg-background/15 px-2 py-0.5 text-[11px] font-medium"
              >
                <span class="size-1.5 flex-none rounded-full {PROVENANCE_DOT[source]}" aria-hidden="true"
                ></span>
                {copy.translations.sourceLabel(source)}
              </span>
              <dl class="flex w-full flex-col gap-0.5 text-[11px]">
                {#if t.translator !== null}
                  <div class="flex w-full gap-2">
                    <dt class="w-[4.5rem] flex-none text-background/60">
                      {copy.translations.tooltipTranslator}
                    </dt>
                    <dd class="min-w-0 flex-1"><bdi>{t.translator}</bdi></dd>
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
                    {offlineLabel}
                  </dt>
                  <!-- TRX-16: availability reads from the verse cache, not the
                       byte size — one-line description stays deferred (no
                       catalogue field to derive it from). -->
                  <dd class="min-w-0 flex-1">{cachedIds.has(t.id)
                      ? downloadedLabel
                      : needsInternetLabel}</dd>
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

        {#if !searchActive && recentRows.length > 0}
          <!-- Pinned above the rail/pane boundary (global on every viewport —
               phones see it at first open, before any language is tapped): the
               reader's most recently used translations across languages; a
               rail's own recents are also boosted to its head below. An
               untouched reader (no recorded use) sees no group and no reorder. -->
          <section
            data-recent-group
            class="row-start-1 flex-none border-b border-border px-2 pb-1 md:col-span-2"
          >
            <h4 class="px-2 pb-1 pt-2.5 text-xs font-medium text-muted-foreground">
              {recentLabel}
            </h4>
            <ul class="flex flex-col gap-0.5">
              {#each recentRows as t (t.id)}
                {@render translationRow(t, true, "recent")}
              {/each}
            </ul>
          </section>
        {/if}
        <nav
          data-language-rail
          aria-label={copy.translations.languagesLabel}
          class="{paneVisible
            ? 'hidden'
            : 'flex'} row-start-2 min-h-0 flex-col gap-0.5 overflow-y-auto overscroll-contain border-border p-2 md:col-start-1 md:flex md:border-e"
        >
          {#each languages as l, i (l.code)}
            {@const active = l.code === activeGroup?.code}
            {#if i > 0 && isSuggested(languages[i - 1]) && !isSuggested(l)}
              <div data-rail-divider class="mx-2.5 my-1.5 h-px flex-none bg-border" aria-hidden="true"></div>
            {/if}
            <!-- One line per language: flag, name, autonym (muted, truncating),
                 an in-use dot when one of its translations is selected, count. -->
            <button
              data-language-option={l.language}
              type="button"
              tabindex={active ? 0 : -1}
              aria-current={active ? "true" : undefined}
              onclick={() => selectLanguage(l.code)}
              onkeydown={(e) => onRailKeydown(e, i)}
              class="flex h-11 flex-none cursor-pointer touch-manipulation items-center gap-2.5 rounded-md px-2.5 text-start transition-colors md:h-9 {active
                ? 'bg-surface-hover text-foreground'
                : 'text-foreground-secondary hover:bg-surface-hover hover:text-foreground'}"
            >
              <span class="flex-none text-base leading-none" aria-hidden="true">{l.flag}</span>
              <span class="flex min-w-0 flex-1 items-baseline gap-1.5">
                <span
                  data-language-name
                  class="flex-none text-[14px] {active ? 'font-medium' : 'font-normal'}"
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
              {#if languagesInUse.has(l.code)}
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
            : 'hidden'} row-start-2 min-h-0 flex-1 touch-manipulation flex-col overflow-x-hidden overflow-y-auto overscroll-contain md:col-start-2 md:flex"
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
                {#each searchGroups as g (g.key)}
                  {#if g.rest.length > 0}
                    {@render translationRow(g.representative, true, "search", {
                      key: g.key,
                      count: g.rest.length + 1,
                      expanded: expandedAuthors.has(g.key),
                    })}
                    {#if expandedAuthors.has(g.key)}
                      {#each g.rest as t (t.id)}
                        {@render translationRow(t, true, "variant")}
                      {/each}
                    {/if}
                  {:else}
                    {@render translationRow(g.representative, true, "search")}
                  {/if}
                {/each}
              </ul>
            {/if}
          {:else if activeGroup}
            <div
              class="sticky top-0 z-10 flex h-12 flex-none items-center gap-2 border-b border-border bg-popover px-4"
            >
              {@render paneBack()}
              <h3 class="text-[15px] font-medium text-foreground">{activeGroup.language}</h3>
              <span data-results-count class="text-[13px] text-muted-foreground">
                {copy.translations.results(paneRowCount)}
              </span>
            </div>
            <ul class="flex flex-col gap-0.5 p-2">
              {#each activeGroups as g (g.key)}
                {#if g.rest.length > 0}
                  {@render translationRow(g.representative, false, "rail", {
                    key: g.key,
                    count: g.rest.length + 1,
                    expanded: expandedAuthors.has(g.key),
                  })}
                  {#if expandedAuthors.has(g.key)}
                    {#each g.rest as t (t.id)}
                      {@render translationRow(t, false, "variant")}
                    {/each}
                  {/if}
                {:else}
                  {@render translationRow(g.representative, false, "rail")}
                {/if}
              {/each}
            </ul>
            {#if commentaryVisible}
              <!-- Commentary (tafsir) rows for this language (TRX-01): stackable
                   like translations, but offered only here — never in the list
                   above, search, recents, or as a reading pick. -->
              <section data-commentary-group class="flex-none border-t border-border px-2 py-1">
                <h4 class="px-2 pb-1 pt-2.5 text-xs font-medium text-muted-foreground">
                  {commentaryLabel}
                </h4>
                <ul class="flex flex-col gap-0.5">
                  {#each commentaryRows as t (t.id)}
                    {@render translationRow(t, false, "tafsir")}
                  {/each}
                </ul>
              </section>
            {/if}
          {/if}
        </section>
      </div>
      </TooltipProvider>

      <div
        class="flex flex-none items-center justify-between gap-3 border-t border-border px-4 py-3 md:px-5"
      >
        <p data-cap-note class="text-xs text-muted-foreground">
          {#if picking}
            {copy.shell.readingPickNote}
          {:else if isFull}
            {copy.stacked.full(STACKED_MAX_EXTRAS)}
          {:else if !stackedTranslations.enabled}
            {copy.stacked.hiddenNote}
          {:else}
            {copy.translations.capNote(STACKED_MAX_EXTRAS)}
          {/if}
        </p>
        <button
          type="button"
          data-done
          onclick={() => (open = false)}
          class="flex h-11 flex-none cursor-pointer touch-manipulation items-center rounded-lg bg-primary px-5 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary-hover md:hidden"
        >
          {copy.translations.done}
        </button>
      </div>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
