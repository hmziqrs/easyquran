<script lang="ts">
  import { appLocale } from "#lib/i18n/app-locale.js";
  import {
    reader_cards_open,
    reader_cards_plus_one,
    reader_cards_target_done,
    reader_zikr_all_done,
    reader_zikr_next,
    reader_zikr_of_target,
    reader_zikr_progress_label,
    reader_zikr_target_announced,
  } from "#lib/i18n/m/reader.js";
  import { publicHref } from "#lib/i18n/public-href.js";
  import { cardDetailHref, cardEntryAttrs, tileCopyFor } from "#lib/home/card-copy.js";
  import { homeCards, type SnippetCard } from "#lib/home/cards.svelte.js";
  import { isTargetDone, nextZikrCard } from "#lib/home/next-zikr.js";
  import type { QuranData } from "#lib/data/quran-data.js";
  import { Icon } from "#lib/components/icon/index.js";

  /**
   * The azkar page's zikr hook: the bound snippet card's live tally, target
   * and recite action, rendered in the card-tile rhythm so the counter feels
   * like the same object the landing strip and /yours show. When the target is
   * reached (or, for a target-less card, once the reader has recited on this
   * visit) the next-zikr row offers the next card on the list — entry-tagged
   * so the destination page binds to that card too. Client-only — the
   * prerendered page never carries personal state.
   */
  let { card, quranData }: { card: SnippetCard; quranData: QuranData | undefined } = $props();

  const locale = appLocale();
  const msgOptions = { locale };

  const done = $derived(isTargetDone(card));

  // Snapshot of the tally at bind time (the parent keys this panel by card id,
  // so a re-bind re-instantiates it): a target-less card reveals the next row
  // only once the reader has recited on this visit.
  // svelte-ignore state_referenced_locally -- the frozen initial read is the point: the arrival count must never track later recites.
  const arrivalCount = card.count;

  // The reach announcement freezes at the crossing count — extra recites past
  // the target must not re-announce a changed {count}; a reset unwinds it.
  let reachedAt = $state<number | null>(null);
  $effect(() => {
    if (done) {
      if (reachedAt === null) reachedAt = card.count;
    } else {
      reachedAt = null;
    }
  });

  function announcedCopy(): string {
    const at = reachedAt;
    const target = card.target;
    if (at === null || target === undefined) return "";
    return reader_zikr_target_announced({ count: at, target }, msgOptions);
  }

  // Next zikr: pure pick over the store list (see ./next-zikr.js); the CTA
  // renders only when the guarded builders give it a real href.
  const next = $derived(nextZikrCard(homeCards.list, { id: card.id, slug: card.slug }));
  const nextView = $derived(next === null ? null : tileCopyFor(next, quranData, locale));
  const nextHref = $derived.by<`/${string}` | null>(() => {
    if (next === null || nextView === null) return null;
    return nextView.href;
  });
  const nextTitle = $derived(next === null || nextView === null ? "" : nextView.title);
  const nextRevealed = $derived(card.target === undefined ? card.count !== arrivalCount : done);
</script>

<div
  class="zikr-panel flex flex-col rounded-md border bg-surface p-4 transition-colors duration-150"
  class:zikr-reached={done}
  class:border-border-strong={done}
  class:border-border={done === false}
>
  <div class="flex items-center justify-between gap-3">
    <!-- The count is the payoff, exactly as on the tile. -->
    <span class="flex items-baseline gap-1.5">
      <span
        class="text-[26px] font-medium leading-none tracking-[-0.01em] text-foreground tabular-nums"
        >{card.count}</span
      >
      {#if card.target !== undefined && done === false}
        <span class="text-[13px] text-muted"
          >{reader_zikr_of_target({ target: card.target }, msgOptions)}</span
        >
      {/if}
    </span>
    {#if done}
      <span class="flex flex-none items-center gap-1 text-[12.5px] font-medium text-primary">
        <Icon name="check" size={14} />
        {reader_cards_target_done(undefined, msgOptions)}
      </span>
    {/if}
    <!-- Card detail is one quiet arrow away; entry attrs keep the session
         thread filed under origin=card like every other card open. -->
    <a
      {...cardEntryAttrs(card)}
      href={publicHref(cardDetailHref(card.id))}
      aria-label={`${reader_cards_open(undefined, msgOptions)}: ${card.slug}`}
      title={reader_cards_open(undefined, msgOptions)}
      class="-me-1 flex size-9 flex-none items-center justify-center rounded-pill text-muted transition-colors hover:bg-surface-hover hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
    >
      <Icon name="arrow-right" size={16} />
    </a>
  </div>

  {#if reachedAt !== null}
    <!-- One announcement per reach, never per tap (a per-tap count in a live
         region is machine-gun chatter for screen readers). -->
    <span role="status" class="sr-only">{announcedCopy()}</span>
  {/if}

  {#if card.target !== undefined}
    {@const fraction = Math.min(1, card.count / card.target)}
    <div
      class="mt-2.5 h-px w-full overflow-hidden rounded-pill bg-surface-hover"
      role="progressbar"
      aria-label={reader_zikr_progress_label(undefined, msgOptions)}
      aria-valuemin={0}
      aria-valuenow={card.count}
      aria-valuemax={card.target}
    >
      <div class="h-full bg-primary" style:width="{Math.round(fraction * 100)}%"></div>
    </div>
  {/if}

  <button
    type="button"
    aria-label={`${reader_cards_plus_one(undefined, msgOptions)}: ${card.slug}`}
    onclick={() => homeCards.increment(card.id)}
    class="mt-3 flex h-11 w-full cursor-pointer touch-manipulation items-center justify-center gap-1.5 rounded-pill border text-[13.5px] font-medium transition-colors duration-150 ease-out focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring active:translate-y-px {done
      ? 'border-transparent bg-primary-soft text-primary hover:bg-primary-hover/10'
      : 'border-border bg-transparent text-foreground-secondary hover:border-border-strong hover:text-foreground'}"
  >
    <Icon name="plus" size={14} />
    {reader_cards_plus_one(undefined, msgOptions)}
  </button>

  {#if nextRevealed && next !== null && nextHref !== null}
    <!-- The next zikr: entry-tagged so the destination binds to that card; the
         href comes from the guarded builders via tileCopyFor, never hand-built. -->
    <a
      {...cardEntryAttrs(next)}
      href={publicHref(nextHref)}
      data-next-zikr
      data-sveltekit-preload-data="hover"
      class="mt-2.5 flex min-h-11 w-full cursor-pointer items-center justify-between gap-2 rounded-pill border px-4 text-[13.5px] font-medium transition-colors duration-150 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring rtl:[&>svg]:-scale-x-100 {done
        ? 'border-transparent bg-primary-soft text-primary hover:bg-primary-hover/10'
        : 'border-border text-foreground-secondary hover:border-border-strong hover:text-foreground'}"
    >
      <span class="min-w-0 truncate">{reader_zikr_next({ title: nextTitle }, msgOptions)}</span>
      <Icon name="arrow-right" size={16} />
    </a>
  {:else if done && next === null}
    <p class="mt-2.5 text-center text-[12.5px] text-muted">
      {reader_zikr_all_done(undefined, msgOptions)}
    </p>
  {/if}
</div>

<style>
  /* The one-shot reach pulse: the reached class lands once per done flip, and
     the keyframe runs a single primary-soft wash over the panel — calm, no
     shake, no sound. The token matches the revealed-ayah wash (layout.css). */
  .zikr-reached {
    animation: zikr-reach 350ms ease-out;
  }

  @keyframes zikr-reach {
    0% {
      background-color: var(--primary-soft);
    }

    100% {
      background-color: transparent;
    }
  }
</style>
