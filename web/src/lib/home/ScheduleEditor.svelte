<script lang="ts">
  import { appLocale } from "#lib/i18n/app-locale.js";
  import {
    reader_cards_schedule,
    reader_cards_schedule_add,
    reader_cards_schedule_empty,
    reader_cards_schedule_full,
    reader_cards_schedule_name,
    reader_cards_schedule_remove,
    reader_cards_schedule_time,
    reader_cards_slot_asr,
    reader_cards_slot_dhuhr,
    reader_cards_slot_evening,
    reader_cards_slot_fajr,
    reader_cards_slot_isha,
    reader_cards_slot_morning,
    reader_cards_slot_maghrib,
  } from "#lib/i18n/m/reader.js";
  import { MAX_SCHEDULE_SLOTS, newCardId } from "#lib/home/cards.svelte.js";
  import { SCHEDULE_PRESETS, type ScheduleSlot } from "#lib/home/schedule.js";

  /**
   * Fixed-clock schedule editor (no geolocation, no astronomy): named
   * `HH:MM` slots a card's due/next hints derive from. Slots are passive
   * reminder hints — nothing here records a recite. Preset chips always
   * insert (duplicates legal, fresh ids); at MAX_SCHEDULE_SLOTS the chips
   * AND the add button hide and the muted cap line shows, matching the
   * collection builder's cap pattern.
   */
  interface Props {
    slots: ScheduleSlot[];
  }

  let { slots = $bindable() }: Props = $props();

  const locale = appLocale();
  const msgOptions = { locale };

  // Preset labels are data strings in schedule.ts; map them onto localized
  // chip copy, falling back to the raw label for an unrecognized one.
  function presetLabel(label: string): string {
    if (label === "Fajr") return reader_cards_slot_fajr(undefined, msgOptions);
    if (label === "Dhuhr") return reader_cards_slot_dhuhr(undefined, msgOptions);
    if (label === "Asr") return reader_cards_slot_asr(undefined, msgOptions);
    if (label === "Maghrib") return reader_cards_slot_maghrib(undefined, msgOptions);
    if (label === "Isha") return reader_cards_slot_isha(undefined, msgOptions);
    if (label === "Morning") return reader_cards_slot_morning(undefined, msgOptions);
    if (label === "Evening") return reader_cards_slot_evening(undefined, msgOptions);
    return label;
  }

  function addSlot(label: string, time: string): void {
    if (slots.length >= MAX_SCHEDULE_SLOTS) return;
    slots = [...slots, { id: newCardId(), label, time }];
  }

  function removeSlot(id: string): void {
    slots = slots.filter((slot) => slot.id !== id);
  }
</script>

<div class="flex flex-col gap-2">
  <span class="eyebrow">{reader_cards_schedule(undefined, msgOptions)}</span>

  {#if slots.length === 0}
    <p class="text-[13px] text-muted">{reader_cards_schedule_empty(undefined, msgOptions)}</p>
  {/if}

  {#if slots.length < MAX_SCHEDULE_SLOTS}
    <div class="flex flex-wrap gap-1.5">
      {#each SCHEDULE_PRESETS as preset (preset.label)}
        <button
          type="button"
          class="flex h-8 items-center rounded-pill border border-border px-3 text-[12.5px] font-medium text-foreground-secondary transition-colors hover:border-border-strong hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
          onclick={() => addSlot(preset.label, preset.time)}
        >
          {presetLabel(preset.label)}
        </button>
      {/each}
    </div>
    <div class="flex">
      <button
        type="button"
        class="flex h-8 items-center rounded-pill border border-border px-3 text-[12.5px] font-medium text-foreground-secondary transition-colors hover:border-border-strong hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
        onclick={() => addSlot("", "")}
      >
        {reader_cards_schedule_add(undefined, msgOptions)}
      </button>
    </div>
  {:else}
    <p class="text-[13px] text-muted">{reader_cards_schedule_full(undefined, msgOptions)}</p>
  {/if}

  {#each slots as slot (slot.id)}
    <div class="flex flex-wrap items-center gap-2">
      <!-- An empty label falls back to the slot's own time at save, so the row
           always persists (decode drops empty labels). -->
      <input
        type="text"
        bind:value={slot.label}
        maxlength="40"
        aria-label={reader_cards_schedule_name(undefined, msgOptions)}
        placeholder={reader_cards_schedule_name(undefined, msgOptions)}
        class="h-9 min-w-0 flex-1 rounded-pill border border-border bg-transparent px-3 text-sm text-foreground placeholder:text-muted focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
      />
      <input
        type="time"
        bind:value={slot.time}
        aria-label={reader_cards_schedule_time(undefined, msgOptions)}
        class="h-9 rounded-pill border border-border bg-transparent px-3 text-sm text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
      />
      <button
        type="button"
        class="flex h-9 w-9 flex-none items-center justify-center rounded-pill border border-border text-foreground-secondary transition-colors hover:border-border-strong hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
        aria-label={reader_cards_schedule_remove(undefined, msgOptions)}
        onclick={() => removeSlot(slot.id)}
      >
        ×
      </button>
    </div>
  {/each}
</div>
