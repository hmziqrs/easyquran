<script lang="ts">
  import { appLocale } from "#lib/i18n/app-locale.js";
  import {
    reader_cards_count_of,
    reader_cards_count_plain,
    reader_cards_plus_one,
    reader_cards_remove,
    reader_cards_reset,
    reader_cards_target_done,
    reader_widget_daily_ayah_open,
  } from "#lib/i18n/m/reader.js";
  import {
    snippetPathFor,
    surahAyahPathFor,
    surahPathFor,
    surahRouteContext,
    type SurahRouteContext,
  } from "#lib/data/quran.js";
  import { publicHref } from "#lib/i18n/public-href.js";
  import { peekTranslationName } from "#lib/quran/catalogue.js";
  import { snippetForSlug, type SnippetText } from "#lib/data/snippets.js";
  import { homeCards, type HomeCard } from "#lib/home/cards.svelte.js";
  import type { QuranData } from "#lib/data/quran-data.js";
  import { Icon } from "#lib/components/index.js";

  /**
   * One home widget card: pinned surah / ayah / range / zikr / azkar set with
   * its tap counter. Shared by the /yours manager and the landing strip — the
   * card carries its own translation context (pinned sourceId, else Arabic),
   * so neither surface has to hydrate the reader's state.
   */
  interface Props {
    card: HomeCard;
    quranData: QuranData | undefined;
    /** Edit-mode extras (remove / reset); the +1 tap works in every mode. */
    editing?: boolean;
  }

  let { card, quranData, editing = false }: Props = $props();

  const locale = appLocale();
  const msgOptions = { locale };

  // Same Arabic fallback the /yours page uses: a personal card with no pinned
  // translation always opens the Arabic mushaf, never a guessed translation.
  const ctx: SurahRouteContext = surahRouteContext("uthmani");

  interface TileCopy {
    readonly title: string;
    readonly arabicTitle: boolean;
    readonly meta: string;
    readonly href: `/${string}` | null;
  }

  function localized(text: SnippetText): string {
    if (locale === "ar") return text.ar;
    return text.en;
  }

  function tile(): TileCopy {
    if (card.kind === "zikr") {
      return { title: card.text, arabicTitle: true, meta: "", href: null };
    }
    if (card.kind === "snippet") {
      const snippet = snippetForSlug(card.slug);
      const title = snippet === null ? card.slug : localized(snippet.title);
      return {
        title,
        arabicTitle: false,
        meta: translationLabel(),
        href: snippetPathFor(ctx, card.slug),
      };
    }
    const surah = quranData?.surahByNum(card.surah);
    if (surah === undefined) {
      return {
        title: `Surah ${card.surah}`,
        arabicTitle: false,
        meta: translationLabel(),
        href: null,
      };
    }
    if (card.kind === "surah") {
      return {
        title: surah.name,
        arabicTitle: false,
        meta: translationLabel(),
        href: surahPathFor(ctx, surah),
      };
    }
    if (card.kind === "ayah") {
      return {
        title: surah.name,
        arabicTitle: false,
        meta: `${card.surah}:${card.ayah} ${translationLabel()}`.trim(),
        href: surahAyahPathFor(ctx, surah, card.ayah),
      };
    }
    return {
      title: surah.name,
      arabicTitle: false,
      meta: `${card.surah}:${card.ayahStart}–${card.ayahEnd} ${translationLabel()}`.trim(),
      href: surahAyahPathFor(ctx, surah, card.ayahStart),
    };
  }

  function translationLabel(): string {
    if (card.kind === "zikr") return "";
    if (card.sourceId === null) return "";
    return peekTranslationName(card.sourceId) ?? "";
  }

  const view = $derived(tile());
  const done = $derived(card.target !== undefined && card.count >= card.target);
  const countText = $derived(
    card.target === undefined
      ? reader_cards_count_plain({ count: card.count }, msgOptions)
      : reader_cards_count_of({ count: card.count, target: card.target }, msgOptions),
  );
  const plusLabel = $derived(`${reader_cards_plus_one(undefined, msgOptions)}: ${view.title}`);
</script>

<div
  class="flex items-center gap-3 rounded-md border border-border bg-surface px-4 py-3 transition-colors"
  class:border-border-strong={done}
>
  <button
    type="button"
    class="flex h-12 w-14 flex-none flex-col items-center justify-center gap-0.5 rounded-md border text-[13px] font-semibold transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
    class:border-primary={done}
    class:text-primary={done}
    class:border-border={done === false}
    class:text-foreground={done === false}
    class:hover:border-border-strong={done === false}
    aria-label={plusLabel}
    onclick={() => homeCards.increment(card.id)}
  >
    {#if done}
      <Icon name="check" size={18} />
    {:else}
      <span class="text-[15px] leading-none">+</span>
      <span class="text-[11px] leading-none font-normal">{card.count}</span>
    {/if}
  </button>

  <span class="min-w-0 flex-1">
    {#if view.arabicTitle}
      <span lang="ar" dir="rtl" class="block truncate font-arabic text-[17px] text-foreground"
        >{view.title}</span
      >
    {:else}
      <span class="block truncate text-sm font-medium text-foreground">{view.title}</span>
    {/if}
    <span class="mt-0.5 flex items-center gap-2 text-[12.5px] text-muted">
      <span class="truncate">{countText}</span>
      {#if view.meta.length > 0}
        <span class="truncate">· {view.meta}</span>
      {/if}
      {#if done}
        <span class="flex-none font-medium text-primary"
          >{reader_cards_target_done(undefined, msgOptions)}</span
        >
      {/if}
    </span>
  </span>

  {#if editing}
    <button
      type="button"
      class="flex h-7 flex-none items-center rounded-pill border border-border px-2.5 text-[11.5px] text-foreground-secondary transition-colors hover:border-border-strong hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
      onclick={() => homeCards.reset(card.id)}
    >
      {reader_cards_reset(undefined, msgOptions)}
    </button>
    <button
      type="button"
      class="flex h-7 flex-none items-center rounded-pill border border-border px-2.5 text-[11.5px] text-danger transition-colors hover:border-border-strong focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
      onclick={() => homeCards.remove(card.id)}
    >
      {reader_cards_remove(undefined, msgOptions)}
    </button>
  {:else if view.href !== null}
    <a
      href={publicHref(view.href)}
      data-sveltekit-preload-data="hover"
      class="flex h-7 flex-none items-center rounded-pill border border-border px-2.5 text-[11.5px] text-foreground-secondary transition-colors hover:border-border-strong hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
    >
      {reader_widget_daily_ayah_open(undefined, msgOptions)}
      <span aria-hidden="true" class="ms-1">→</span>
    </a>
  {/if}
</div>
