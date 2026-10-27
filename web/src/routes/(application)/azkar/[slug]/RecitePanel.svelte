<script lang="ts">
  import { appLocale } from "#lib/i18n/app-locale.js";
  import {
    reader_cards_open,
    reader_cards_plus_one,
    reader_cards_target_done,
  } from "#lib/i18n/m/reader.js";
  import { publicHref } from "#lib/i18n/public-href.js";
  import { cardDetailHref, cardEntryAttrs } from "#lib/home/card-copy.js";
  import { homeCards, type SnippetCard } from "#lib/home/cards.svelte.js";
  import { Icon } from "#lib/components/icon/index.js";

  /**
   * The azkar page's zikr hook: the matched snippet card's tally, target and
   * recite action, rendered in the card-tile rhythm so the counter feels like
   * the same object the landing strip and /yours show. Client-only — the
   * prerendered page never carries personal state.
   */
  let { card }: { card: SnippetCard } = $props();

  const locale = appLocale();
  const msgOptions = { locale };

  const done = $derived(card.target !== undefined && card.count >= card.target);
</script>

<div
  class="flex flex-col rounded-md border bg-surface p-4 transition-colors"
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
        <span class="text-[13px] text-muted">of {card.target}</span>
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

  {#if card.target !== undefined}
    {@const fraction = Math.min(1, card.count / card.target)}
    <div
      class="mt-2.5 h-px w-full overflow-hidden rounded-pill bg-surface-hover"
      role="progressbar"
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
</div>
