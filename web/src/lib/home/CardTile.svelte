<script lang="ts">
  import * as DropdownMenu from "#lib/components/ui/dropdown-menu/index.js";
  import { goto } from "$app/navigation";

  import { appLocale } from "#lib/i18n/app-locale.js";
  import {
    reader_cards_duplicate,
    reader_cards_edit_title,
    reader_cards_full,
    reader_cards_menu,
    reader_cards_open,
    reader_cards_plus_one,
    reader_cards_remove,
    reader_cards_reset,
    reader_cards_target_done,
    reader_cards_kind_surah,
    reader_widget_daily_ayah_open,
  } from "#lib/i18n/m/reader.js";
  import { publicHref } from "#lib/i18n/public-href.js";
  import { homeCards, type HomeCard } from "#lib/home/cards.svelte.js";
  import { cardDetailHref, scheduleBadgeCopy, cardEntryAttrs, tileCopyFor } from "#lib/home/card-copy.js";
  import { armCardUndo } from "#lib/home/card-undo.svelte.js";
  import { slotStateFor } from "#lib/home/schedule.js";
  import type { QuranData } from "#lib/data/quran-data.js";
  import { Icon } from "#lib/components/index.js";
  import { iconButtonVariants } from "#lib/components/ui/icon-button/icon-button-variants.js";

  /**
   * One home widget card: pinned surah / ayah / range / zikr / azkar set /
   * collection with its tap counter, optional target progress, due/next
   * schedule hint and — on manager surfaces — the shadcn dropdown-menu of
   * per-card actions (detail / edit / duplicate / reset / remove). Shared by
   * the /yours grid and the landing strip: the strip passes neither `menu`
   * nor `onEdit`, so it keeps a quiet arrow affordance and the card carries
   * its own translation context (pinned sourceId, else Arabic), never the
   * reader's live state.
   *
   * Anatomy (one rhythm for every kind): title row, meta + schedule line,
   * the count as the visual hero, a hairline progress mark, then one honest
   * labeled recite action — no unlabeled submit slab, no duplicated count.
   */
  interface Props {
    card: HomeCard;
    quranData: QuranData | undefined;
    /** Manager surface: the actions menu replaces the arrow affordance. */
    menu?: boolean;
    /** Edit-card handoff; the menu hides the item when absent. */
    onEdit?: (card: HomeCard) => void;
  }

  let { card, quranData, menu = false, onEdit }: Props = $props();

  const locale = appLocale();
  const msgOptions = { locale };

  const view = $derived(tileCopyFor(card, quranData, locale));
  const done = $derived(card.target !== undefined && card.count >= card.target);

  // A surah card whose meta is just the word "Surah" restates the title; hide
  // it. Kind labels stay for zikr ("Zikr" names an Arabic phrase) and for
  // cards whose meta carries coordinates or a pinned translation.
  const metaVisible = $derived(
    view.meta.length > 0 &&
      !(card.kind === "surah" && view.meta === reader_cards_kind_surah(undefined, msgOptions)),
  );

  interface ScheduleBadge {
    readonly due: boolean;
    readonly label: string;
  }

  /* Badge precedence: a finished card is never nagged; otherwise the schedule's
     due/next slot state (±30-minute inclusive window, midnight wrap handled in
     schedule.ts; label formatting shared with the detail page). Slots are
     passive hints — they never write count or events. */
  const badge = $derived.by<ScheduleBadge | null>(() => {
    if (done) return null;
    const schedule = card.schedule;
    if (schedule === undefined || schedule.length === 0) return null;
    const state = slotStateFor(schedule, Date.now());
    return state === null ? null : scheduleBadgeCopy(state, locale);
  });

  /* ── Actions menu (shadcn dropdown-menu over bits-ui: keyboard nav, escape,
     outside click and focus return are the primitive's job) ──────────────── */
  let duplicateFull = $state(false);

  function openDetails(): void {
    void goto(publicHref(cardDetailHref(card.id)));
  }

  function editCard(): void {
    const handler = onEdit;
    if (handler === undefined) return;
    handler(card);
  }

  function duplicateCard(event: Event): void {
    // Refused (store full): preventDefault keeps the menu open so the
    // refusal line stays visible next to the items.
    if (homeCards.duplicate(card.id) === false) {
      duplicateFull = true;
      event.preventDefault();
    }
  }

  function resetCard(): void {
    if (card.count === 0) return;
    homeCards.reset(card.id);
  }

  function removeCard(): void {
    // The trigger unmounts with the tile; the section-level undo banner takes
    // the focus from here (card-undo is module-owned, so the banner also works
    // when removal happens on the detail page).
    armCardUndo(card);
    homeCards.remove(card.id);
  }

  const plusLabel = $derived(`${reader_cards_plus_one(undefined, msgOptions)}: ${view.title}`);
</script>

<div
  class="relative flex flex-col rounded-md border bg-surface p-4 transition-colors"
  class:border-border-strong={done}
  class:border-border={done === false}
>
  <!-- Title row. The trailing control is one quiet arrow (strip) or the actions
       menu (/yours) — never a second link duplicating the title's. -->
  <div class="flex items-start gap-2">
    <span class="min-w-0 flex-1">
      {#if view.href !== null}
        <a
          {...cardEntryAttrs(card)}
          href={publicHref(view.href)}
          data-sveltekit-preload-data="hover"
          class="block truncate rounded-sm text-[15px] font-medium leading-6 text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
        >
          {view.title}
        </a>
      {:else if view.arabicTitle}
        <!-- The phrase is the card's content: Arabic display size, with the
             reader-grade 2.1 headroom so Scheherazade's harakat ink stays
             inside the line on iOS. -->
        <span
          lang="ar"
          dir="rtl"
          class="block truncate font-arabic text-[21px] leading-[2.1] text-foreground"
          >{view.title}</span
        >
      {:else}
        <span class="block truncate text-[15px] font-medium leading-6 text-foreground"
          >{view.title}</span
        >
      {/if}
    </span>

    {#if menu}
      <DropdownMenu.Root>
        <DropdownMenu.Trigger
          aria-label={reader_cards_menu(undefined, msgOptions)}
          class="-me-1 -mt-1 {iconButtonVariants({ variant: "ghost", size: "sm" })}"
        >
          <Icon name="dots" size={18} />
        </DropdownMenu.Trigger>
        <DropdownMenu.Content sideOffset={4} align="end">
          {#if duplicateFull}
            <p role="alert" class="px-2.5 py-1.5 text-[12px] text-danger">
              {reader_cards_full(undefined, msgOptions)}
            </p>
          {/if}
          <DropdownMenu.Group>
            <DropdownMenu.Item onSelect={openDetails}>
              {reader_cards_open(undefined, msgOptions)}
            </DropdownMenu.Item>
            {#if onEdit !== undefined}
              <DropdownMenu.Item onSelect={editCard}>
                {reader_cards_edit_title(undefined, msgOptions)}
              </DropdownMenu.Item>
            {/if}
            <DropdownMenu.Item onSelect={duplicateCard}>
              {reader_cards_duplicate(undefined, msgOptions)}
            </DropdownMenu.Item>
          </DropdownMenu.Group>
          <DropdownMenu.Separator />
          <DropdownMenu.Group>
            <DropdownMenu.Item disabled={card.count === 0} onSelect={resetCard}>
              {reader_cards_reset(undefined, msgOptions)}
            </DropdownMenu.Item>
            <DropdownMenu.Item variant="destructive" onSelect={removeCard}>
              {reader_cards_remove(undefined, msgOptions)}
            </DropdownMenu.Item>
          </DropdownMenu.Group>
        </DropdownMenu.Content>
      </DropdownMenu.Root>
    {:else if view.href !== null}
      <a
        {...cardEntryAttrs(card)}
        href={publicHref(view.href)}
        data-sveltekit-preload-data="hover"
        aria-label={`${reader_widget_daily_ayah_open(undefined, msgOptions)}: ${view.title}`}
        title={reader_widget_daily_ayah_open(undefined, msgOptions)}
        class="-me-1.5 -mt-1 flex size-9 flex-none items-center justify-center rounded-pill text-muted transition-colors hover:bg-surface-hover hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring rtl:[&>svg]:-scale-x-100"
      >
        <Icon name="arrow-right" size={16} />
      </a>
    {/if}
  </div>

  <!-- Meta + schedule hint on one line; both quiet, badge right-aligned. -->
  {#if metaVisible || badge !== null}
    <div class="mt-0.5 flex min-h-6 items-center justify-between gap-2">
      {#if metaVisible}
        <span class="min-w-0 truncate text-[12.5px] text-muted">{view.meta}</span>
      {:else}
        <span class="min-w-0 flex-1"></span>
      {/if}
      {#if badge !== null}
        <span
          class="flex h-6 flex-none items-center rounded-pill border px-2 text-[11.5px] font-medium"
          class:border-border-strong={badge.due}
          class:text-primary={badge.due}
          class:border-border={badge.due === false}
          class:text-muted={badge.due === false}
        >
          {badge.label}
        </span>
      {/if}
    </div>
  {/if}

  <!-- The count is the payoff of a counter card: set it large. Past-target
       reads "40 · ✓ Target reached" — never the broken "40 of 33". -->
  <div class="mt-1 flex min-h-7 items-baseline justify-between gap-2">
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
  </div>

  {#if card.target !== undefined}
    {@const fraction = Math.min(1, card.count / card.target)}
    <!-- Hairline mark, not a slab: one pixel of fill reads as progress without
         turning the card into a dashboard. -->
    <div
      class="mt-2.5 h-px w-full overflow-hidden rounded-pill bg-surface-hover"
      role="progressbar"
      aria-valuenow={card.count}
      aria-valuemax={card.target}
    >
      <div class="h-full bg-primary" style:width="{Math.round(fraction * 100)}%"></div>
    </div>
  {/if}

  <!-- One honest action: labeled, single-line, quiet ghost until the target is
       met, then a soft primary wash. Still tappable past the target — dhikr
       keeps counting — so it never poses as a terminal "submit" state. -->
  <button
    type="button"
    aria-label={plusLabel}
    onclick={() => homeCards.increment(card.id)}
    class="mt-3 flex h-11 w-full cursor-pointer touch-manipulation items-center justify-center gap-1.5 rounded-pill border text-[13.5px] font-medium transition-colors duration-150 ease-out focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring active:translate-y-px {done
      ? 'border-transparent bg-primary-soft text-primary hover:bg-primary-hover/10'
      : 'border-border bg-transparent text-foreground-secondary hover:border-border-strong hover:text-foreground'}"
  >
    <Icon name="plus" size={14} />
    {reader_cards_plus_one(undefined, msgOptions)}
  </button>
</div>
