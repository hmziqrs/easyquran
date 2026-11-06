<script lang="ts">
  import { authState } from "#lib/auth/auth-state.svelte.js";
  import type { SettingsCopy } from "#lib/i18n/settings-copy.js";
  import { loadReminder, saveReminder } from "#lib/reminders/api.js";
  import {
    clockFromMinutes,
    DEFAULT_REMINDER_MINUTE,
    deviceTimeZone,
    minutesFromClock,
    scheduleProblem,
    type ReminderSchedule,
  } from "#lib/reminders/schedule.js";
  import { notifications } from "#lib/stores/notifications.svelte.js";
  import { cn } from "#lib/utils.js";

  let {
    copy,
    onLabel,
    offLabel,
  }: {
    copy: SettingsCopy["privacy"]["reminders"];
    onLabel: string;
    offLabel: string;
  } = $props();

  const toggle = "rounded-pill border px-3.5 py-2.5 text-caption transition-colors duration-150";
  const on = "border-primary bg-primary-soft text-foreground";
  const off = "border-border-strong text-foreground-secondary hover:border-border hover:text-foreground";
  const field =
    "rounded-md border border-border-strong bg-surface px-2.5 py-1.5 text-caption text-foreground";

  const signedIn = $derived(authState.status === "authenticated");

  // The last schedule the server confirmed. The form below is a draft of it.
  let saved = $state.raw<ReminderSchedule | null>(null);
  let loadFailed = $state(false);
  let busy = $state(false);
  let problemText = $state<string | null>(null);

  let reminderClock = $state(clockFromMinutes(DEFAULT_REMINDER_MINUTE));
  let quietOn = $state(false);
  let quietFromClock = $state("22:00");
  let quietUntilClock = $state("07:00");

  const enabled = $derived(saved?.enabled ?? false);
  const zone = $derived(saved?.timeZone ?? deviceTimeZone());

  // Plain flag, not state: the load runs once per signed-in visit.
  let requested = false;

  $effect(() => {
    if (!signedIn || requested) return;
    requested = true;
    void load();
  });

  function applySchedule(schedule: ReminderSchedule): void {
    saved = schedule;
    reminderClock = clockFromMinutes(schedule.reminderMinute);
    quietOn = schedule.quietStartMinute !== null && schedule.quietEndMinute !== null;
    if (schedule.quietStartMinute !== null) quietFromClock = clockFromMinutes(schedule.quietStartMinute);
    if (schedule.quietEndMinute !== null) quietUntilClock = clockFromMinutes(schedule.quietEndMinute);
  }

  async function load(): Promise<void> {
    const res = await loadReminder();
    if (!res.ok) {
      loadFailed = true;
      return;
    }
    if (res.schedule !== null) applySchedule(res.schedule);
  }

  /** The form as a schedule, or null when a time field is not a valid clock value. */
  function draft(nextEnabled: boolean): ReminderSchedule | null {
    const reminderMinute = minutesFromClock(reminderClock);
    if (reminderMinute === null) return null;
    const quietStartMinute = quietOn ? minutesFromClock(quietFromClock) : null;
    const quietEndMinute = quietOn ? minutesFromClock(quietUntilClock) : null;
    if (quietOn && (quietStartMinute === null || quietEndMinute === null)) return null;
    return {
      enabled: nextEnabled,
      timeZone: zone,
      reminderMinute,
      quietStartMinute,
      quietEndMinute,
    };
  }

  /** Saves the draft with the given enabled state. Keeps the last good state on failure. */
  async function persist(nextEnabled: boolean): Promise<void> {
    problemText = null;
    const next = draft(nextEnabled);
    if (next === null) {
      problemText = copy.invalid;
      return;
    }
    if (scheduleProblem(next) !== null) {
      problemText = copy.invalid;
      return;
    }
    busy = true;
    try {
      const res = await saveReminder(next);
      if (res.ok) {
        applySchedule(res.schedule);
      } else {
        problemText = copy.error;
      }
    } finally {
      busy = false;
    }
  }

  /** Turning reminders on needs push first. The notifications row shows why when it fails. */
  async function onToggleEnabled(): Promise<void> {
    if (busy) return;
    if (!enabled) {
      busy = true;
      let subscribed = false;
      try {
        subscribed = notifications.subscribed || (await notifications.subscribe());
      } finally {
        busy = false;
      }
      if (!subscribed) {
        problemText = copy.needsPush;
        return;
      }
    }
    await persist(!enabled);
  }

  function onQuietToggle(): void {
    if (busy) return;
    quietOn = !quietOn;
    if (quietOn) return;
    void persist(enabled);
  }
</script>

<div class="px-4 py-3.5 sm:px-5">
  <div class="flex flex-wrap items-center justify-between gap-4">
    <div class="min-w-0">
      <h3 class="text-[14.5px] font-medium text-foreground">{copy.heading}</h3>
      <p class="mt-0.5 text-caption leading-snug text-muted">{copy.intro}</p>
    </div>
    {#if signedIn}
      <button
        type="button"
        aria-pressed={enabled}
        disabled={busy}
        onclick={onToggleEnabled}
        class={cn(toggle, "shrink-0", enabled ? on : off, busy && "cursor-not-allowed opacity-50")}
      >
        {enabled ? onLabel : offLabel}
      </button>
    {/if}
  </div>

  {#if !signedIn}
    <p class="mt-2 text-caption leading-snug text-muted">{copy.signedOut}</p>
  {:else}
    {#if loadFailed}
      <p role="alert" class="mt-2 text-caption text-destructive">{copy.loadError}</p>
    {/if}

    <div class="mt-3 flex flex-wrap items-end gap-x-6 gap-y-3">
      <label class="flex flex-col gap-1 text-caption text-muted" for="goal-reminder-time">
        {copy.time}
        <input
          id="goal-reminder-time"
          type="time"
          bind:value={reminderClock}
          disabled={busy}
          onchange={() => void persist(enabled)}
          class={field}
        />
      </label>

      <button
        type="button"
        aria-pressed={quietOn}
        disabled={busy}
        onclick={onQuietToggle}
        class={cn(toggle, "shrink-0", quietOn ? on : off)}
      >
        {copy.quiet}
      </button>

      {#if quietOn}
        <label class="flex flex-col gap-1 text-caption text-muted" for="goal-reminder-quiet-from">
          {copy.quietFrom}
          <input
            id="goal-reminder-quiet-from"
            type="time"
            bind:value={quietFromClock}
            disabled={busy}
            onchange={() => void persist(enabled)}
            class={field}
          />
        </label>
        <label class="flex flex-col gap-1 text-caption text-muted" for="goal-reminder-quiet-until">
          {copy.quietUntil}
          <input
            id="goal-reminder-quiet-until"
            type="time"
            bind:value={quietUntilClock}
            disabled={busy}
            onchange={() => void persist(enabled)}
            class={field}
          />
        </label>
      {/if}
    </div>

    <p class="mt-2 text-caption leading-snug text-muted">{copy.zone(zone)}</p>
    {#if problemText !== null}
      <p role="alert" class="mt-2 text-caption text-destructive">{problemText}</p>
    {/if}
  {/if}
</div>
