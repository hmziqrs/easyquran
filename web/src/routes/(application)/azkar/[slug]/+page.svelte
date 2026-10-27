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
    surahAyahPathFor,
    toArabicDigits,
  } from "#lib/data/quran.js";
  import type { QuranData } from "#lib/data/quran-data.js";
  import { loadQuranData, peekQuranData } from "#lib/data/quran-data-client.js";
  import { TRANSLATION_CATALOGUE } from "#lib/quran/catalogue.js";
  import { SNIPPET_KIND_LABELS, type SnippetText } from "#lib/data/snippets.js";
  import { getReaderUiCopy } from "#lib/i18n/reader-copy.js";
  import { readerHrefFor } from "#lib/i18n/reader.js";
  import { publicHref } from "#lib/i18n/public-href.js";
  import { reader } from "#lib/stores/reader.svelte.js";
  import { stackedTranslations } from "#lib/stores/stacked-translations.svelte.js";
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
  import StackedLanes from "../../_reader/StackedLanes.svelte";
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
    return publicHref(readerHrefFor(locale, surahAyahPathFor(ctx, surah, group.ayahStart)));
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

<ReaderShell textTools={false} translationsTool {onStackChanged}>
  {#snippet header()}
    <h1 class="sr-only">{title}</h1>
    <span class="ms-auto rounded-pill border border-border-strong px-2 py-0.5 text-[11.5px] text-muted"
      >{kindLabel()}</span
    >
  {/snippet}

  <div class="mx-auto flex max-w-3xl flex-col gap-8 py-8">
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
      <section class="flex flex-col gap-4" aria-labelledby="snippet-ref-{group.surah}">
        <div class="flex flex-wrap items-baseline justify-between gap-2 border-b border-reader-divider pb-2">
          <h3 id="snippet-ref-{group.surah}" class="text-[15px] font-medium text-foreground">
            {group.name}
            <span lang="ar" dir="rtl" class="ms-2 text-foreground-secondary">{group.arabicName}</span>
          </h3>
          <a
            href={readerHref(group)}
            class="text-[12.5px] text-primary underline-offset-4 hover:underline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
          >
            {extentLabel(group)} →
          </a>
        </div>

        {#each group.ayahs as row (row.key)}
          <div class="flex flex-col">
            <p
              dir="rtl"
              lang="ar"
              data-verse-anchor={row.key}
              class="font-arabic text-[1.6rem] leading-[2.15] text-quran-foreground"
            >
              {row.text}<span
                class="ms-2 align-middle font-sans text-[0.62em] text-foreground-secondary"
                >﴿{toArabicDigits(row.ayah)}﴾</span
              >
            </p>
            {#if controller !== undefined}
              <StackedLanes
                stacked={stackedFor(controller.state, row.key)}
                stackedPending={loadingFor(controller.state, row.key)}
                stackedErrored={erroredFor(controller.state, row.key)}
                errorLabel={copy.stacked.error}
              />
            {/if}
          </div>
        {/each}
      </section>
    {/each}
  </div>
</ReaderShell>
