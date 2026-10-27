<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { toast } from "svelte-sonner";
  import { Seo } from "#lib/components/index.js";
  import { Icon } from "#lib/components/icon/index.js";
  import { bookmarks } from "#lib/bookmarks/store.svelte.js";
  import {
    resumeCtxFor,
    snippetPath,
    surahAyahRevealPathFor,
  } from "#lib/data/quran.js";
  import { QuranScript } from "#lib/data/quran-types.js";
  import type { QuranData } from "#lib/data/quran-data.js";
  import { loadQuranData, peekQuranData } from "#lib/data/quran-data-client.js";
  import { TRANSLATION_CATALOGUE, TRANSLATION_CATALOGUE_BY_ID } from "#lib/quran/catalogue.js";
  import { SNIPPET_KIND_LABELS, type SnippetText } from "#lib/data/snippets.js";
  import { getReaderUiCopy } from "#lib/i18n/reader-copy.js";
  import { readerHrefFor } from "#lib/i18n/reader.js";
  import { publicHref } from "#lib/i18n/public-href.js";
  import { reader } from "#lib/stores/reader.svelte.js";
  import { readingText } from "#lib/stores/reading-text.svelte.js";
  import { stackedTranslations } from "#lib/stores/stacked-translations.svelte.js";
  import { readingFlowId } from "#lib/reader/reading-flow.js";
  import {
    reader_cards_add,
    reader_cards_added,
    reader_cards_full,
    reader_cards_set_target,
    reader_cards_translation_offer,
    reader_cards_translation_update,
    reader_cards_updated,
  } from "#lib/i18n/m/reader.js";
  import { cardDetailHref } from "#lib/home/card-copy.js";
  import { homeCards, newCardId, nowStamp, type SnippetCard } from "#lib/home/cards.svelte.js";
  import type { ShareVerse } from "#lib/stores/reader-share.svelte.js";
  import { revealVerseMarker, trackReaderThread } from "#lib/sessions/reader-thread.svelte.js";
  import ReaderShell from "../../_reader/ReaderShell.svelte";
  import VerseRow from "../../_reader/VerseRow.svelte";
  import {
    createStackedTranslations,
    erroredFor,
    loadingFor,
    stackedFor,
    type StackedController,
  } from "../../_reader/stacked-translations.svelte.js";
  import { ayahIndexValidator } from "../../_reader/range-validate.js";
  import RecitePanel from "./RecitePanel.svelte";
  import type { SnippetRefGroup } from "./+page.server";

  let { data } = $props();

  const copy = getReaderUiCopy();
  const locale = copy.locale;
  const msgOptions = { locale };
  // Prerendered HTML is Arabic-only; after hydration the "open in reader" link
  // honors the reader's active translation, like every index page.
  const ctx = $derived(resumeCtxFor(reader.lastRead, { kind: "arabic" }));

  function localized(text: SnippetText): string {
    if (locale === "ar") return text.ar;
    return text.en;
  }

  function kindLabel(): string {
    return localized(SNIPPET_KIND_LABELS[data.kind]);
  }

  function readerHref(group: SnippetRefGroup): string {
    const surah = { slug: group.slug, num: group.surah };
    // Reveal-mirror form: this page prerenders, so the ayah fragment link
    // (whose target rows only exist after hydration) would fail the build.
    return publicHref(readerHrefFor(locale, surahAyahRevealPathFor(ctx, surah, group.ayahStart)));
  }

  function extentLabel(group: SnippetRefGroup): string {
    if (group.ayahStart === group.ayahEnd) return `${group.surah}:${group.ayahStart}`;
    return `${group.surah}:${group.ayahStart}-${group.ayahEnd}`;
  }

  const title = $derived(localized(data.title));
  const description = $derived(localized(data.description));
  const canonicalPath = $derived(snippetPath(data.slug));

  /* ── Zikr card hook ────────────────────────────────────────────────────────
     The snippet page is a card surface: a matching snippet card turns the page
     into a live counter (recite panel), and translation changes here can
     re-pin the card's translation. All personal state is client-only — the
     prerendered HTML never carries a card, a count, or a translation. */
  let mounted = $state(false);
  const matchedCard = $derived(
    mounted
      ? (homeCards.list.find(
          (card): card is SnippetCard => card.kind === "snippet" && card.slug === data.slug,
        ) ?? null)
      : null,
  );

  /** The translation the reader is effectively reading with right now: the
   * stack head when the flow is on, Arabic (null) when paused or empty. */
  function effectiveSourceId(): string | null {
    return stackedTranslations.enabled ? (stackedTranslations.ids[0] ?? null) : null;
  }

  const OFFER_TOAST_ID = "azkar-card-translation-offer";
  function onStackChanged(): void {
    const card = matchedCard;
    if (card === null) return;
    const effective = effectiveSourceId();
    if (effective === card.sourceId) return;
    // Enable/disable both land here: an empty or paused stack offers Arabic
    // (null), a new head offers that translation. Dismiss = keep as-is.
    toast(reader_cards_translation_offer(undefined, msgOptions), {
      id: OFFER_TOAST_ID,
      duration: 12000,
      action: {
        label: reader_cards_translation_update(undefined, msgOptions),
        onClick: () => {
          homeCards.setSource(card.id, effective);
          toast(reader_cards_updated(undefined, msgOptions), { duration: 4000 });
        },
      },
    });
  }

  function pinCard(): void {
    const card: SnippetCard = {
      id: newCardId(),
      kind: "snippet",
      slug: data.slug,
      sourceId: effectiveSourceId(),
      count: 0,
      createdAt: nowStamp(),
    };
    if (homeCards.add(card) === false) {
      toast(reader_cards_full(undefined, msgOptions), { duration: 6000 });
      return;
    }
    toast(reader_cards_added(undefined, msgOptions), {
      duration: 8000,
      action: {
        label: reader_cards_set_target(undefined, msgOptions),
        onClick: () => void goto(publicHref(cardDetailHref(card.id))),
      },
    });
  }

  /* ── Stacked translations ──────────────────────────────────────────────────
     One controller per ref group: snippet refs are per-surah spans, and one
     wide min..max range would fetch every verse between disjoint groups. The
     controllers exist only after hydration — the prerendered page stays
     Arabic-only (quran-system.md Part 5). */
  let quranData = $state.raw<QuranData | undefined>(peekQuranData());
  let controllers = $state.raw<StackedController[]>([]);

  onMount(() => {
    mounted = true;
    homeCards.hydrate();
    if (peekQuranData() === undefined) {
      void loadQuranData().then(
        (loaded) => {
          quranData = loaded;
        },
        () => undefined,
      );
    }
  });

  $effect(() => {
    const loaded = quranData;
    if (loaded === undefined) return;
    controllers = data.refs.map((group) => {
      const start = loaded.globalIndexOf(group.surah, group.ayahStart);
      const end = loaded.globalIndexOf(group.surah, group.ayahEnd);
      // Server load already validated every ref (bad curated refs 500 there).
      const from = start ?? 0;
      const to = end ?? 0;
      return createStackedTranslations({
        from: () => from,
        to: () => to,
        validator: () => ayahIndexValidator(loaded),
        routeSourceId: () => null,
        catalogue: () => TRANSLATION_CATALOGUE,
        routeKey: () => data.slug,
      });
    });
  });

  // sync() reads the stacked store, so the effect re-fetches when the reader
  // adds, removes or reorders translations — same loop the reader routes run.
  $effect(() => {
    for (const controller of controllers) controller.sync();
  });

  /* ── Reading mode ──────────────────────────────────────────────────────────
     The real reader's Reading flow, ported: the flowed translation is the
     reading preference, else the stack head (readingFlowId). A flow outside
     the stack gets its own one-id fetcher, exactly like SurahReader's
     readingController. */
  const flowId = $derived(
    reader.isReadingMode
      ? readingFlowId(readingText.text, readingText.translationId, null, null, stackedTranslations.ids)
      : null,
  );
  const flowFromStack = $derived(flowId !== null && stackedTranslations.ids.includes(flowId));
  const readingFetchIds = $derived(flowId !== null && flowFromStack === false ? [flowId] : []);
  // The flow read spans the whole snippet: min..max across refs (multi-ref
  // snippets over-fetch the gap verses — one small read, same trade the
  // reader's whole-range makes).
  interface FlowRange {
    from: number;
    to: number;
  }

  function flowRange(): FlowRange {
    const loaded = quranData;
    if (loaded === undefined) return { from: 0, to: 0 };
    let from = Number.MAX_SAFE_INTEGER;
    let to = 0;
    for (const group of data.refs) {
      const start = loaded.globalIndexOf(group.surah, group.ayahStart);
      const end = loaded.globalIndexOf(group.surah, group.ayahEnd);
      if (start !== undefined && start < from) from = start;
      if (end !== undefined && end > to) to = end;
    }
    return to > 0 ? { from, to } : { from: 0, to: 0 };
  }
  const readingController = createStackedTranslations({
    from: () => flowRange().from,
    to: () => flowRange().to,
    validator: () => (quranData ? ayahIndexValidator(quranData) : null),
    routeSourceId: () => null,
    catalogue: () => TRANSLATION_CATALOGUE,
    routeKey: () => data.slug,
    ids: () => readingFetchIds,
  });
  $effect(() => readingController.sync());
  onDestroy(() => readingController.dispose());

  const flowEntry = $derived(
    flowId === null ? undefined : TRANSLATION_CATALOGUE_BY_ID.get(flowId),
  );

  /** What one verse row shows: Arabic + lanes (Ayah-by-Ayah), or the flowed
   * translation inline (Reading) — the SurahReader rowView split. */
  interface RowView {
    text: string;
    isTranslation: boolean | undefined;
    pending: boolean;
  }

  function rowView(row: SnippetRefGroup["ayahs"][number]): RowView {
    if (reader.isReadingMode && flowId !== null) {
      const flowed = flowText(row.key);
      return { text: flowed ?? "", isTranslation: true, pending: flowed === null };
    }
    return { text: row.text, isTranslation: undefined, pending: false };
  }

  function flowText(key: string): string | null {
    for (const controller of controllers) {
      const fromStack = stackedFor(controller.state, key).find((t) => t.sourceId === flowId);
      if (fromStack) return fromStack.text;
    }
    return stackedFor(readingController.state, key).find((t) => t.sourceId === flowId)?.text ?? null;
  }

  onDestroy(() => {
    for (const controller of controllers) controller.dispose();
  });

  // Every snippet ref files as its own range row in the default folder.
  const rangeBookmarkable = $derived(data.refs.length > 0);

  // A snippet is a bounded reading thread: reading its last verse completes it.
  trackReaderThread({
    key: () => data.slug,
    target: () => {
      const first = data.refs[0];
      const last = data.refs.at(-1);
      if (first === undefined || last === undefined) return null;
      return {
        scope: `snippet:${data.slug}`,
        surah: first.surah,
        ayah: first.ayahStart,
        exact: false,
        boundEnd: `${last.surah}:${last.ayahEnd}`,
        sourceId: null,
      };
    },
    onResume: (thread) => void revealVerseMarker(thread.surah, thread.ayah),
  });
  const allRefsMarked = $derived(
    data.refs.every((group) =>
      bookmarks.isRangeMarked(group.surah, group.ayahStart, group.surah, group.ayahEnd),
    ),
  );

  function onBookmarkRange(): void {
    if (!rangeBookmarkable) return;
    // All marked → this click removes every range; otherwise fill in the gaps.
    for (const group of data.refs) {
      const marked = bookmarks.isRangeMarked(
        group.surah,
        group.ayahStart,
        group.surah,
        group.ayahEnd,
      );
      if (allRefsMarked || !marked) {
        bookmarks.toggleRange(group.surah, group.ayahStart, group.surah, group.ayahEnd);
      }
    }
  }

  // Share (FLOW-05 range variant): verse texts joined + canonical `?ayah=` URL.
  const FEEDBACK_MS = 4000;
  let shareNote = $state("");
  let shareTimer: ReturnType<typeof setTimeout> | null = null;

  function flashShareNote(message: string): void {
    shareNote = message;
    if (shareTimer) clearTimeout(shareTimer);
    shareTimer = setTimeout(() => (shareNote = ""), FEEDBACK_MS);
  }

  async function onShareRange(): Promise<void> {
    const verses: ShareVerse[] = [];
    for (const group of data.refs) {
      for (const row of group.ayahs) verses.push({ key: row.key, text: row.text });
    }
    const first = verses[0];
    const last = verses[verses.length - 1];
    if (first === undefined || last === undefined) return;
    const result = await reader.shareRange(first.key, last.key, verses);
    if (result === "copied") {
      flashShareNote(copy.verse.copied);
      return;
    }
    if (result === "link-copied") flashShareNote(copy.verse.shareFailed);
    else if (result === "failed") flashShareNote(copy.verse.shareUnavailable);
  }

  onDestroy(() => {
    if (shareTimer) clearTimeout(shareTimer);
  });

  const actionButton =
    "inline-flex h-8 items-center gap-1.5 rounded-lg border border-border-strong px-2.5 text-[12.5px] text-foreground-secondary transition-colors hover:border-border hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring";
</script>

<Seo path={canonicalPath} title={title} description={description} includeTextVariants={false} />

<ReaderShell textTools {onStackChanged}>
  {#snippet header()}
    <h1 class="sr-only">{title}</h1>
    <span class="ms-auto rounded-pill border border-border-strong px-2 py-0.5 text-[11.5px] text-muted"
      >{kindLabel()}</span
    >
  {/snippet}

  <!-- Same frame as the reader column: full Container width, so the rows,
       sizes and lanes match a surah page exactly. -->
  <div class="flex flex-col gap-8 py-8">
    <header class="flex flex-col gap-2">
      <h2 class="text-xl font-semibold text-foreground">{title}</h2>
      <p class="text-[14px] leading-relaxed text-foreground-secondary">{description}</p>
      <div class="mt-1 flex flex-wrap items-center gap-2">
        {#if rangeBookmarkable}
          <button
            type="button"
            class={actionButton}
            aria-pressed={allRefsMarked}
            onclick={onBookmarkRange}
          >
            <Icon name={allRefsMarked ? "bookmark-fill" : "bookmark"} size={14} />
            {copy.verse.bookmarkRange}
          </button>
        {/if}
        <button type="button" class={actionButton} onclick={onShareRange}>
          <Icon name="share" size={14} />
          {copy.verse.share}
        </button>
        {#if shareNote}
          <span role="status" class="text-[12px] text-muted">{shareNote}</span>
        {/if}
      </div>
    </header>

    {#if mounted}
      {#if matchedCard !== null}
        <RecitePanel card={matchedCard} />
      {:else}
        <!-- No zikr card for this set yet: pin one, carrying whatever
             translation the reader is currently on. -->
        <button
          type="button"
          data-pin-card
          onclick={pinCard}
          class="flex h-11 w-full cursor-pointer touch-manipulation items-center justify-center gap-1.5 rounded-pill border border-border text-[13.5px] font-medium text-foreground-secondary transition-colors duration-150 hover:border-border-strong hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring active:translate-y-px"
        >
          <Icon name="plus" size={14} />
          {reader_cards_add(undefined, msgOptions)}
        </button>
      {/if}
    {/if}

    {#each data.refs as group, gi (group.surah)}
      {@const controller = controllers[gi]}
      <section aria-labelledby="snippet-ref-{group.surah}">
        <!-- Same section frame as the range reader: surah rule, Arabic name,
             jump to the full surah — the azkar set reads like its surahs. -->
        <div class="flex items-center justify-between gap-3 border-b border-reader-divider py-3">
          <h3
            id="snippet-ref-{group.surah}"
            class="text-sm font-medium text-foreground"
          >
            {group.name}
            <span lang="ar" dir="rtl" class="ms-2 font-arabic text-base text-foreground-secondary"
              >{group.arabicName}</span
            >
          </h3>
          <a
            href={readerHref(group)}
            class="flex flex-none items-center gap-2 text-[12.5px] text-primary transition-colors hover:brightness-110 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
          >
            <span>{extentLabel(group)}</span>
            <span aria-hidden="true">→</span>
          </a>
        </div>

        <!-- The reader's own list and rows: reader typography, script, sizing,
             per-verse tools, stacked lanes, Reading flow. -->
        <div data-source-kind={reader.isReadingMode && flowId !== null ? "translation" : "arabic"}>
          <ol class="ayah-list flex list-none flex-col p-0">
            {#each group.ayahs as row (row.key)}
              {@const view = rowView(row)}
              <VerseRow
                text={view.text}
                n={row.ayah}
                vKey={row.key}
                isTranslation={view.isTranslation}
                translationLang={flowEntry?.languageCode}
                translationDirection={flowEntry?.direction}
                translationName={flowEntry?.name ?? undefined}
                pending={view.pending}
                script={QuranScript.Uthmani}
                stacked={controller === undefined
                  ? []
                  : stackedFor(controller.state, row.key)}
                stackedPending={controller === undefined
                  ? []
                  : loadingFor(controller.state, row.key)}
                stackedErrored={controller === undefined
                  ? []
                  : erroredFor(controller.state, row.key)}
                stackedErrorLabel={copy.stacked.error}
              />
            {/each}
          </ol>
        </div>
      </section>
    {/each}
  </div>
</ReaderShell>
