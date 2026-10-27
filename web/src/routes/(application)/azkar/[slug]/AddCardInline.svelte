<script lang="ts">
  import { tick } from "svelte";
  import { toast } from "svelte-sonner";

  import { appLocale } from "#lib/i18n/app-locale.js";
  import {
    reader_cards_add,
    reader_cards_added,
    reader_cards_cancel,
    reader_cards_full,
    reader_cards_target_label,
    reader_zikr_add_confirm,
    reader_zikr_target_down,
    reader_zikr_target_up,
  } from "#lib/i18n/m/reader.js";
  import { homeCards, newCardId, nowStamp, type SnippetCard } from "#lib/home/cards.svelte.js";
  import { Icon } from "#lib/components/icon/index.js";

  /**
   * The azkar page's inline add-card, as a disclosure: the "Add card" pill
   * stays mounted and expands the mini-form beneath it — an optional target
   * stepper and one Add action — so a card is created on the page itself,
   * pinned to the translation the reader is on right now (the caller's
   * `getSourceId`) and bound explicitly via `onAdded(id)`. No toast-hop to the
   * detail page; the panel's matched card swaps to a recite tally on its own.
   * Client-only — the prerendered page never carries the form or a card.
   */
  let {
    slug,
    getSourceId,
    onAdded,
  }: {
    slug: string;
    getSourceId: () => string | null;
    /** Fires with the created card's id; the page binds the panel to it explicitly. */
    onAdded?: (id: string) => void;
  } = $props();

  const locale = appLocale();
  const msgOptions = { locale };

  const FORM_ID = "azkar-add-card-form";
  /** Same bounds as homeCards.setTarget / the detail page's CardForm. */
  const TARGET_MIN = 1;
  const TARGET_MAX = 10_000;
  /** First tap on an unset stepper — the common tasbih count. */
  const TARGET_FIRST = 33;

  let open = $state(false);
  let storeFull = $state(false);
  let target: number | undefined = $state(undefined);
  let pill = $state<HTMLButtonElement | null>(null);
  // The stepper's "+" is the first enabled control (− is disabled while the
  // target is unset), so it receives the opening focus.
  let stepUp = $state<HTMLButtonElement | null>(null);

  function openForm(): void {
    storeFull = false;
    open = true;
    void tick().then(() => stepUp?.focus());
  }

  // Cancel and Escape land here: collapse + focus back to the pill (same
  // contract as the detail page's edit form — the pill never unmounts, so the
  // focus return is real). Successful Add collapses without the focus hop:
  // this whole surface is about to be replaced by the recite panel.
  function closeForm(): void {
    open = false;
    target = undefined;
    pill?.focus();
  }

  function collapse(): void {
    open = false;
    target = undefined;
  }

  function toggleForm(): void {
    if (open) closeForm();
    else openForm();
  }

  function increase(): void {
    if (target === undefined) {
      target = TARGET_FIRST;
      return;
    }
    target = Math.min(TARGET_MAX, target + 1);
  }

  function decrease(): void {
    if (target === undefined) return;
    target = Math.max(TARGET_MIN, target - 1);
  }

  function addCard(): void {
    const card: SnippetCard = {
      id: newCardId(),
      kind: "snippet",
      slug,
      sourceId: getSourceId(),
      count: 0,
      target,
      createdAt: nowStamp(),
    };
    if (homeCards.add(card) === false) {
      // Refusal stays inline — the form stays open, no toast, no navigation.
      storeFull = true;
      return;
    }
    toast(reader_cards_added(undefined, msgOptions), { duration: 4000 });
    onAdded?.(card.id);
    collapse();
  }

  function valueLabel(): string {
    if (target === undefined) return "—";
    return String(target);
  }

  // Escape dismisses from anywhere while the form is open — the dialog-dismiss
  // idiom for a transient inline form (the sidebar/Tweaks <svelte:window>
  // precedent), not an app shortcut, so it stays out of the Hotkeys registry.
  // IME sessions keep their own Escape; never steal it mid-composition.
  function onWindowKeydown(event: KeyboardEvent): void {
    if (!open || event.isComposing) return;
    if (event.key === "Escape") {
      event.preventDefault();
      closeForm();
    }
  }
</script>

<svelte:window onkeydown={onWindowKeydown} />

<!-- Disclosure pill: stays mounted so aria-expanded is observable true while
     the form is open and focus can return here on close. -->
<button
  type="button"
  bind:this={pill}
  data-add-card
  aria-expanded={open}
  aria-controls={FORM_ID}
  onclick={toggleForm}
  class="flex h-11 w-full cursor-pointer touch-manipulation items-center justify-center gap-1.5 rounded-pill border text-[13.5px] font-medium transition-colors duration-150 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring active:translate-y-px {open
    ? 'border-border-strong text-foreground'
    : 'border-border text-foreground-secondary hover:border-border-strong hover:text-foreground'}"
>
  <Icon name="plus" size={14} />
  {reader_cards_add(undefined, msgOptions)}
</button>

<div id={FORM_ID} hidden={!open} class="mt-2 flex flex-col rounded-md border border-border bg-surface p-4">
  <div class="flex items-center justify-between gap-3">
    <span class="text-[12.5px] font-medium text-foreground-secondary"
      >{reader_cards_target_label(undefined, msgOptions)}</span
    >
    <span class="text-[26px] font-medium leading-none tracking-[-0.01em] text-foreground tabular-nums"
      >{valueLabel()}</span
    >
  </div>
  <div class="mt-3 flex items-center gap-2">
    <button
      type="button"
      aria-label={reader_zikr_target_down(undefined, msgOptions)}
      disabled={target === undefined || target <= TARGET_MIN}
      onclick={decrease}
      class="flex h-11 w-11 flex-none cursor-pointer touch-manipulation items-center justify-center rounded-pill border border-border text-foreground-secondary transition-colors duration-150 hover:border-border-strong hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring active:translate-y-px disabled:pointer-events-none disabled:opacity-40"
    >
      <Icon name="minus" size={16} />
    </button>
    <button
      type="button"
      bind:this={stepUp}
      aria-label={reader_zikr_target_up(undefined, msgOptions)}
      disabled={target !== undefined && target >= TARGET_MAX}
      onclick={increase}
      class="flex h-11 w-11 flex-none cursor-pointer touch-manipulation items-center justify-center rounded-pill border border-border text-foreground-secondary transition-colors duration-150 hover:border-border-strong hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring active:translate-y-px disabled:pointer-events-none disabled:opacity-40"
    >
      <Icon name="plus" size={16} />
    </button>
    <button
      type="button"
      onclick={addCard}
      class="ms-auto flex h-11 min-w-24 flex-1 cursor-pointer touch-manipulation items-center justify-center gap-1.5 rounded-pill border border-border px-4 text-[13.5px] font-medium text-foreground-secondary transition-colors duration-150 hover:border-border-strong hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring active:translate-y-px sm:flex-none"
    >
      {reader_zikr_add_confirm(undefined, msgOptions)}
    </button>
    <button
      type="button"
      onclick={closeForm}
      class="flex h-11 flex-none cursor-pointer touch-manipulation items-center rounded-pill px-3 text-[13.5px] font-medium text-muted transition-colors duration-150 hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
    >
      {reader_cards_cancel(undefined, msgOptions)}
    </button>
  </div>
  {#if storeFull}
    <p role="alert" class="mt-2.5 text-[12.5px] text-danger">
      {reader_cards_full(undefined, msgOptions)}
    </p>
  {/if}
</div>
