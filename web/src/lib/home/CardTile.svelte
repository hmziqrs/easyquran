<script lang="ts">
  import { DropdownMenu } from "bits-ui";
  import { goto } from "$app/navigation";

  import { appLocale } from "#lib/i18n/app-locale.js";  import {
    reader_cards_count_of,
    reader_cards_count_plain,
    reader_cards_duplicate,
    reader_cards_edit_title,
    reader_cards_full,
    reader_cards_menu,
    reader_cards_open,
    reader_cards_plus_one,
    reader_cards_remove,
    reader_cards_reset,
    reader_cards_target_done,
    reader_widget_daily_ayah_open,
  } from "#lib/i18n/m/reader.js";
  import { publicHref } from "#lib/i18n/public-href.js";
  import { homeCards, type HomeCard } from "#lib/home/cards.svelte.js";
  import {
    cardDetailHref,
    passesCountLabel,
    scheduleBadgeCopy,
    tileCopyFor,
  } from "#lib/home/card-copy.js";
  import { armCardUndo } from "#lib/home/card-undo.svelte.js";
  import { slotStateFor } from "#lib/home/schedule.js";
  import type { QuranData } from "#lib/data/quran-data.js";
  import { Icon } from "#lib/components/index.js";
  import { Button } from "#lib/components/ui/button/index.js";
  import { iconButtonVariants } from "#lib/components/ui/icon-button/icon-button-variants.js";

  /**
   * One home widget card: pinned surah / ayah / range / zikr / azkar set /
   * collection with its tap counter, optional target progress, due/next
   * schedule hint and — on manager surfaces — a bits-ui DropdownMenu of
   * per-card actions (detail / edit / duplicate / reset / remove). Shared by
   * the /yours grid and the landing strip: the strip passes neither `menu`
   * nor `onEdit`, so it keeps its plain Open pill and the card carries its
   * own translation context (pinned sourceId, else Arabic), never the
   * reader's live state.
   */
  interface Props {
    card: HomeCard;
    quranData: QuranData | undefined;
    /** Manager surface: the actions menu replaces the Open pill. */
    menu?: boolean;
    /** Edit-card handoff; the menu hides the item when absent. */
    onEdit?: (card: HomeCard) => void;
  }

  let { card, quranData, menu = false, onEdit }: Props = $props();

  const locale = appLocale();
  const msgOptions = { locale };

  const view = $derived(tileCopyFor(card, quranData, locale));
  const done = $derived(card.target !== undefined && card.count >= card.target);

  const countText = $derived.by<string>(() => {
    if (card.kind === "collection") return passesCountLabel(card.count, locale);
    if (card.target === undefined) {
      return reader_cards_count_plain({ count: card.count }, msgOptions);
    }
    return reader_cards_count_of({ count: card.count, target: card.target }, msgOptions);
  });
  const plusLabel = $derived(`${reader_cards_plus_one(undefined, msgOptions)}: ${view.title}`);

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

  /* ── Actions menu (bits-ui DropdownMenu: keyboard nav, escape, outside
     click and focus return are the primitive's job) ───────────────────────── */
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
    // focus from here (card-undo is module-owned, so the banner also works
    // when removal happens on the detail page).
    armCardUndo(card);
    homeCards.remove(card.id);
  }

  const menuItemClass =
    "flex w-full cursor-default select-none items-center rounded-sm px-2.5 py-1.5 text-start text-[13px] text-foreground-secondary outline-none transition-colors data-[highlighted]:bg-surface-hover data-[highlighted]:text-foreground data-[disabled]:pointer-events-none data-[disabled]:opacity-40";
</script>

<div
  class="relative flex flex-col gap-2 rounded-md border border-border bg-surface p-4 transition-colors"
  class:border-border-strong={done}
>
  <div class="flex items-start gap-2">
    <span class="min-w-0 flex-1">
      {#if view.href !== null}
        <a
          href={publicHref(view.href)}
          data-sveltekit-preload-data="hover"
          class="block truncate rounded-sm text-sm font-medium text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
        >
          {view.title}
        </a>
      {:else if view.arabicTitle}
        <span lang="ar" dir="rtl" class="block truncate font-arabic text-[17px] text-foreground"
          >{view.title}</span
        >
      {:else}
        <span class="block truncate text-sm font-medium text-foreground">{view.title}</span>
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
        <DropdownMenu.Content
          sideOffset={4}
          align="end"
          class="z-20 flex min-w-[10rem] flex-col rounded-md border border-border-strong bg-surface p-1 shadow-lg"
        >
          {#if duplicateFull}
            <p role="alert" class="px-2.5 py-1.5 text-[12px] text-danger">
              {reader_cards_full(undefined, msgOptions)}
            </p>
          {/if}
          <DropdownMenu.Item class={menuItemClass} onSelect={openDetails}>
            {reader_cards_open(undefined, msgOptions)}
          </DropdownMenu.Item>
          {#if onEdit !== undefined}
            <DropdownMenu.Item class={menuItemClass} onSelect={editCard}>
              {reader_cards_edit_title(undefined, msgOptions)}
            </DropdownMenu.Item>
          {/if}
          <DropdownMenu.Item class={menuItemClass} onSelect={duplicateCard}>
            {reader_cards_duplicate(undefined, msgOptions)}
          </DropdownMenu.Item>
          <DropdownMenu.Separator class="-mx-1 my-1 h-px bg-border" />
          <DropdownMenu.Item
            class={menuItemClass}
            disabled={card.count === 0}
            onSelect={resetCard}
          >
            {reader_cards_reset(undefined, msgOptions)}
          </DropdownMenu.Item>
          <DropdownMenu.Item class="{menuItemClass} text-danger data-[highlighted]:text-danger" onSelect={removeCard}>
            {reader_cards_remove(undefined, msgOptions)}
          </DropdownMenu.Item>
        </DropdownMenu.Content>
      </DropdownMenu.Root>
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

  {#if view.meta.length > 0}
    <span class="truncate text-[12.5px] text-muted">{view.meta}</span>
  {/if}

  {#if badge !== null}
    <span class="flex">
      <span
        class="flex h-6 items-center rounded-pill border px-2 text-[11.5px] font-medium"
        class:border-border-strong={badge.due}
        class:text-primary={badge.due}
        class:border-border={badge.due === false}
        class:text-muted={badge.due === false}
      >
        {badge.label}
      </span>
    </span>
  {/if}

  <span class="flex flex-wrap items-center gap-2 text-[12.5px]">
    <span class="truncate text-foreground-secondary">{countText}</span>
    {#if done}
      <span class="flex-none font-medium text-primary"
        >{reader_cards_target_done(undefined, msgOptions)}</span
      >
    {/if}
  </span>
  {#if card.target !== undefined}
    {@const fraction = Math.min(1, card.count / card.target)}
    <div
      class="h-1.5 w-full overflow-hidden rounded-pill bg-surface-hover"
      role="progressbar"
      aria-valuenow={card.count}
      aria-valuemax={card.target}
    >
      <div class="h-full rounded-pill bg-primary" style:width="{Math.round(fraction * 100)}%"></div>
    </div>
  {/if}

  <Button
    type="button"
    variant={done ? "primary" : "secondary"}
    class="w-full flex-col gap-0.5 py-2"
    aria-label={plusLabel}
    onclick={() => homeCards.increment(card.id)}
  >
    {#if done}
      <Icon name="check" size={18} />
    {:else}
      <span class="text-[15px] leading-none">+</span>
      <span class="text-[11px] leading-none font-normal">{card.count}</span>
    {/if}
  </Button>
</div>
