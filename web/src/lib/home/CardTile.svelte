<script lang="ts">
  import { appLocale } from "#lib/i18n/app-locale.js";
  import {
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

  /**
   * One home widget card: pinned surah / ayah / range / zikr / azkar set /
   * collection with its tap counter, optional target progress, due/next
   * schedule hint and — on manager surfaces — a per-card actions panel
   * (detail / edit / duplicate / reset / remove). Shared by the /yours grid
   * and the landing strip: the strip passes neither `menu` nor `onEdit`, so it
   * keeps its plain Open pill and the card carries its own translation
   * context (pinned sourceId, else Arabic), never the reader's live state.
   */
  interface Props {
    card: HomeCard;
    quranData: QuranData | undefined;
    /** Manager surface: the actions panel replaces the Open pill. */
    menu?: boolean;
    /** Edit-card handoff; the panel hides the item when absent. */
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

  /* ── Actions panel (ARIA disclosure, not a menu widget) ──────────────────── */
  let panelOpen = $state(false);
  let duplicateFull = $state(false);
  let rootEl = $state<HTMLDivElement>();
  let triggerEl = $state<HTMLButtonElement>();

  const panelId = $derived(`card-actions-${card.id}`);

  function togglePanel(): void {
    if (panelOpen) {
      panelOpen = false;
      triggerEl?.focus();
      return;
    }
    duplicateFull = false;
    panelOpen = true;
  }

  function closePanel(): void {
    panelOpen = false;
    triggerEl?.focus();
  }

  // Escape closes the panel through the shared hotkey registry (dynamically
  // imported so @tanstack/hotkeys never rides the landing strip's bundle).
  $effect(() => {
    if (!panelOpen) return;
    let destroyed = false;
    let handle: { unregister(): void } | undefined;
    void import("#lib/hotkeys.svelte.js").then(({ registerHotkey }) => {
      if (destroyed) return;
      handle = registerHotkey(
        "Escape",
        (event) => {
          // IME: a composition session's Escape belongs to the IME, not us.
          if (event.isComposing) return;
          closePanel();
        },
        { meta: { name: "Close card actions panel" } },
      );
    });
    return () => {
      destroyed = true;
      handle?.unregister();
    };
  });

  $effect(() => {
    if (!panelOpen) return;
    const onPointerDown = (event: PointerEvent) => {
      const root = rootEl;
      if (root !== undefined && event.target instanceof Node && !root.contains(event.target)) {
        panelOpen = false;
      }
    };
    document.addEventListener("pointerdown", onPointerDown, true);
    return () => document.removeEventListener("pointerdown", onPointerDown, true);
  });

  function openDetails(): void {
    panelOpen = false;
  }

  function editCard(): void {
    const handler = onEdit;
    if (handler === undefined) return;
    panelOpen = false;
    handler(card);
  }

  function duplicateCard(): void {
    // Refused (store full): the panel STAYS open and shows the refusal line.
    if (homeCards.duplicate(card.id) === false) {
      duplicateFull = true;
      return;
    }
    closePanel();
  }

  function resetCard(): void {
    if (card.count === 0) return;
    homeCards.reset(card.id);
    closePanel();
  }

  function removeCard(): void {
    // The trigger unmounts with the tile; the section-level undo banner takes
    // focus from here (card-undo is module-owned, so the banner also works
    // when removal happens on the detail page).
    armCardUndo(card);
    homeCards.remove(card.id);
  }
</script>

<div
  bind:this={rootEl}
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
      <button
        type="button"
        bind:this={triggerEl}
        aria-expanded={panelOpen}
        aria-controls={panelId}
        aria-label={reader_cards_menu(undefined, msgOptions)}
        class="flex h-11 w-11 flex-none items-center justify-center rounded-md text-muted transition-colors hover:bg-surface-hover hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
        onclick={togglePanel}
      >
        <Icon name="dots" size={18} />
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

  <button
    type="button"
    class="flex min-h-11 w-full flex-none flex-col items-center justify-center gap-0.5 rounded-pill border text-[13px] font-semibold transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
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

  {#if menu && panelOpen}
    <div
      id={panelId}
      class="absolute end-0 top-[calc(100%+4px)] z-20 flex min-w-[10rem] flex-col rounded-md border border-border-strong bg-surface p-1"
    >
      {#if duplicateFull}
        <p role="alert" class="px-2.5 py-1.5 text-[12px] text-danger">
          {reader_cards_full(undefined, msgOptions)}
        </p>
      {/if}
      <a
        href={publicHref(cardDetailHref(card.id))}
        data-sveltekit-preload-data="hover"
        class="flex w-full items-center rounded-sm px-2.5 py-1.5 text-start text-[13px] text-foreground-secondary transition-colors hover:bg-surface-hover hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
        onclick={openDetails}
      >
        {reader_cards_open(undefined, msgOptions)}
      </a>
      {#if onEdit !== undefined}
        <button
          type="button"
          class="flex w-full items-center rounded-sm px-2.5 py-1.5 text-start text-[13px] text-foreground-secondary transition-colors hover:bg-surface-hover hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
          onclick={editCard}
        >
          {reader_cards_edit_title(undefined, msgOptions)}
        </button>
      {/if}
      <button
        type="button"
        class="flex w-full items-center rounded-sm px-2.5 py-1.5 text-start text-[13px] text-foreground-secondary transition-colors hover:bg-surface-hover hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
        onclick={duplicateCard}
      >
        {reader_cards_duplicate(undefined, msgOptions)}
      </button>
      <button
        type="button"
        aria-disabled={card.count === 0}
        class="flex w-full items-center rounded-sm px-2.5 py-1.5 text-start text-[13px] text-foreground-secondary transition-colors hover:bg-surface-hover hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
        class:opacity-40={card.count === 0}
        onclick={resetCard}
      >
        {reader_cards_reset(undefined, msgOptions)}
      </button>
      <button
        type="button"
        class="flex w-full items-center rounded-sm px-2.5 py-1.5 text-start text-[13px] text-danger transition-colors hover:bg-surface-hover focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
        onclick={removeCard}
      >
        {reader_cards_remove(undefined, msgOptions)}
      </button>
    </div>
  {/if}
</div>
