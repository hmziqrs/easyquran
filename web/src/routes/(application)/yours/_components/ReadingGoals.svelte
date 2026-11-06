<script lang="ts">
  import { onMount } from "svelte";
  import { readingAnalytics } from "#lib/analytics/reading-analytics.js";
  import { authState } from "#lib/auth/auth-state.svelte.js";
  import {
    GOAL_PRESETS,
    addDays,
    calendarDays,
    draftError,
    goalProgress,
    presetDraft,
    streakOf,
    windowEnd,
    type GoalDraftError,
    type GoalKind,
    type GoalPreset,
    type GoalProgress,
    type GoalState,
    type ReadingDay,
    type ReadingGoal,
  } from "#lib/goals/goal-progress.js";
  import { goals } from "#lib/goals/store.svelte.js";
  import type { UiLocale } from "#lib/i18n/locales.js";
  import {
    reader_goals_calendar,
    reader_goals_create,
    reader_goals_custom_heading,
    reader_goals_day_none,
    reader_goals_day_read,
    reader_goals_end_label,
    reader_goals_error_dates,
    reader_goals_error_limit,
    reader_goals_error_minutes,
    reader_goals_error_span,
    reader_goals_kind,
    reader_goals_kind_finish,
    reader_goals_kind_minutes,
    reader_goals_local_note,
    reader_goals_minutes_label,
    reader_goals_minutes_note,
    reader_goals_none,
    reader_goals_pause,
    reader_goals_per_day,
    reader_goals_preset_finish_30,
    reader_goals_preset_finish_365,
    reader_goals_preset_minutes,
    reader_goals_progress_minutes,
    reader_goals_progress_verses,
    reader_goals_remove,
    reader_goals_resume,
    reader_goals_start_heading,
    reader_goals_start_label,
    reader_goals_state_active,
    reader_goals_state_completed,
    reader_goals_state_ended,
    reader_goals_state_paused,
    reader_goals_streak,
    reader_goals_title,
    reader_goals_unavailable,
    reader_goals_window,
  } from "#lib/i18n/m/reader.js";
  import { preferences } from "#lib/preferences/store.svelte.js";
  import { todayLocal } from "#lib/quran/engagement-state.js";

  // Goals and the streak calendar. Days come from the server rollup when signed
  // in (every device counts), the device rollup otherwise. A failed load is shown
  // as unavailable, never as zero reads.
  let { locale }: { locale: UiLocale } = $props();

  const msg = $derived({ locale });
  const HISTORY_DAYS = 400;
  const CALENDAR_WEEKS = 5;

  let today = $state(todayLocal(preferences.now()));
  let days = $state.raw<readonly ReadingDay[] | null>(null);
  let loaded = $state(false);
  let error = $state<GoalDraftError | null>(null);

  let customKind = $state<GoalKind>("daily_minutes");
  let customMinutes = $state(15);
  let customStart = $state("");
  let customEnd = $state("");

  const signedIn = $derived(authState.status === "authenticated" && authState.user !== null);
  const list = $derived(goals.entries);
  const streak = $derived(days === null ? 0 : streakOf(days, today));
  const readDays = $derived(new Set((days ?? []).filter((d) => d.ayahsRead > 0).map((d) => d.localDay)));
  const cells = $derived(calendarDays(today, CALENDAR_WEEKS));

  async function refresh(): Promise<void> {
    today = todayLocal(preferences.now());
    const result = await readingAnalytics.fetchDays(addDays(today, -(HISTORY_DAYS - 1)), today);
    days = result;
    loaded = true;
  }

  onMount(() => {
    customStart = todayLocal(preferences.now());
    customEnd = addDays(todayLocal(preferences.now()), 29);
    void refresh();
  });

  function progressOf(goal: ReadingGoal): GoalProgress | null {
    if (days === null) return null;
    return goalProgress(goal, days, today);
  }

  function errorText(problem: GoalDraftError): string {
    if (problem === "dates") return reader_goals_error_dates(undefined, msg);
    if (problem === "span") return reader_goals_error_span(undefined, msg);
    if (problem === "minutes") return reader_goals_error_minutes(undefined, msg);
    return reader_goals_error_limit(undefined, msg);
  }

  function stateText(state: GoalState): string {
    if (state === "paused") return reader_goals_state_paused(undefined, msg);
    if (state === "completed") return reader_goals_state_completed(undefined, msg);
    if (state === "ended") return reader_goals_state_ended(undefined, msg);
    return reader_goals_state_active(undefined, msg);
  }

  function presetLabel(preset: GoalPreset): string {
    if (preset === "minutes_30") return reader_goals_preset_minutes(undefined, msg);
    if (preset === "finish_30") return reader_goals_preset_finish_30(undefined, msg);
    return reader_goals_preset_finish_365(undefined, msg);
  }

  function startPreset(preset: GoalPreset): void {
    error = goals.add(presetDraft(preset, today));
  }

  function addCustom(): void {
    error = goals.add({
      kind: customKind,
      dailyMinutes: customKind === "daily_minutes" ? customMinutes : null,
      startDay: customStart,
      endDay: customEnd,
    });
  }

  // The draft check runs before the form submits, so the same message shows here.
  const customProblem = $derived(
    draftError(
      {
        kind: customKind,
        dailyMinutes: customKind === "daily_minutes" ? customMinutes : null,
        startDay: customStart,
        endDay: customEnd,
      },
      list.length,
    ),
  );

  function togglePause(goal: ReadingGoal): void {
    if (goal.pausedFrom === null) goals.pause(goal.id, today);
    else goals.resume(goal.id, today);
  }

  function percent(done: number, target: number): number {
    if (target <= 0) return 0;
    return Math.min(100, Math.round((done / target) * 100));
  }

  function progressText(goal: ReadingGoal, progress: GoalProgress): string {
    const done = Math.round(progress.done).toLocaleString(locale);
    const target = Math.round(progress.target).toLocaleString(locale);
    if (goal.kind === "finish_quran") {
      return reader_goals_progress_verses({ done, target }, msg);
    }
    return reader_goals_progress_minutes({ done, target }, msg);
  }

  function dayLabel(day: string): string {
    return `${day}: ${readDays.has(day) ? reader_goals_day_read(undefined, msg) : reader_goals_day_none(undefined, msg)}`;
  }
</script>

<section class="mt-6 rounded-md border border-border bg-surface px-4 py-3" aria-labelledby="goals-heading">
  <h3 id="goals-heading" class="eyebrow">{reader_goals_title(undefined, msg)}</h3>

  {#if !signedIn}
    <p class="mt-2 text-[13px] text-muted">{reader_goals_local_note(undefined, msg)}</p>
  {/if}

  {#if loaded && days === null}
    <p class="mt-2 text-[13px] text-danger" role="status">{reader_goals_unavailable(undefined, msg)}</p>
  {:else if !loaded}
    <p class="mt-2 text-[13px] text-muted" aria-busy="true">…</p>
  {:else}
    <p class="mt-2 text-sm text-foreground">{reader_goals_streak({ count: streak }, msg)}</p>

    <div class="mt-3">
      <h4 class="text-[13px] text-muted">{reader_goals_calendar(undefined, msg)}</h4>
      <ol class="mt-2 grid grid-cols-7 gap-1" aria-label={reader_goals_calendar(undefined, msg)}>
        {#each cells as day (day)}
          <li
            title={dayLabel(day)}
            aria-label={dayLabel(day)}
            class="flex aspect-square items-center justify-center rounded-sm text-[11px] tabular-nums {readDays.has(day) ? 'bg-accent text-accent-foreground' : 'bg-muted-bg text-muted'}"
          >
            {Number(day.slice(8, 10))}
          </li>
        {/each}
      </ol>
    </div>

    <div class="mt-4">
      {#if list.length === 0}
        <p class="text-[13px] text-muted">{reader_goals_none(undefined, msg)}</p>
      {:else}
        <ul class="flex flex-col gap-3">
          {#each list as goal (goal.id)}
            {@const progress = progressOf(goal)}
            <li class="rounded-md border border-border px-3 py-2.5">
              <div class="flex flex-wrap items-baseline justify-between gap-2">
                <span class="text-sm text-foreground">
                  {goal.kind === "finish_quran"
                    ? reader_goals_kind_finish(undefined, msg)
                    : reader_goals_per_day({ minutes: goal.dailyMinutes ?? 0 }, msg)}
                </span>
                {#if progress !== null}
                  <span class="text-[12px] text-muted">{stateText(progress.state)}</span>
                {/if}
              </div>
              <p class="mt-0.5 text-[12px] text-muted">
                {reader_goals_window({ start: goal.startDay, end: windowEnd(goal, today) }, msg)}
              </p>
              {#if progress !== null}
                <div
                  role="progressbar"
                  aria-label={progressText(goal, progress)}
                  aria-valuemin={0}
                  aria-valuemax={Math.round(progress.target)}
                  aria-valuenow={Math.round(Math.min(progress.done, progress.target))}
                  class="mt-2 h-1.5 w-full overflow-hidden rounded-full bg-muted-bg"
                >
                  <div class="h-full rounded-full bg-accent" style="width: {percent(progress.done, progress.target)}%"></div>
                </div>
                <p class="mt-1 text-[12px] tabular-nums text-muted">{progressText(goal, progress)}</p>
              {/if}
              {#if goal.kind === "daily_minutes"}
                <p class="mt-1 text-[11px] text-muted">{reader_goals_minutes_note(undefined, msg)}</p>
              {/if}
              <div class="mt-2 flex gap-2">
                <button
                  type="button"
                  class="rounded-md border border-border-strong px-2.5 py-1 text-[12px] text-foreground-secondary hover:text-foreground"
                  onclick={() => togglePause(goal)}
                >
                  {goal.pausedFrom === null ? reader_goals_pause(undefined, msg) : reader_goals_resume(undefined, msg)}
                </button>
                <button
                  type="button"
                  class="rounded-md border border-border-strong px-2.5 py-1 text-[12px] text-foreground-secondary hover:text-foreground"
                  onclick={() => goals.remove(goal.id)}
                >
                  {reader_goals_remove(undefined, msg)}
                </button>
              </div>
            </li>
          {/each}
        </ul>
      {/if}
    </div>

    <div class="mt-4 border-t border-border pt-3">
      <h4 class="text-[13px] text-muted">{reader_goals_start_heading(undefined, msg)}</h4>
      <div class="mt-2 flex flex-wrap gap-2">
        {#each GOAL_PRESETS as preset (preset)}
          <button
            type="button"
            class="rounded-md border border-border-strong px-2.5 py-1.5 text-[12px] text-foreground-secondary hover:text-foreground"
            onclick={() => startPreset(preset)}
          >
            {presetLabel(preset)}
          </button>
        {/each}
      </div>

      <form
        class="mt-4 grid gap-2.5 sm:grid-cols-2"
        onsubmit={(event) => {
          event.preventDefault();
          addCustom();
        }}
      >
        <p class="text-[13px] text-muted sm:col-span-2">{reader_goals_custom_heading(undefined, msg)}</p>
        <label class="flex flex-col gap-1 text-[12px] text-muted">
          {reader_goals_kind(undefined, msg)}
          <select bind:value={customKind} class="rounded-md border border-border-strong bg-surface px-2 py-1.5 text-sm text-foreground">
            <option value="daily_minutes">{reader_goals_kind_minutes(undefined, msg)}</option>
            <option value="finish_quran">{reader_goals_kind_finish(undefined, msg)}</option>
          </select>
        </label>
        {#if customKind === "daily_minutes"}
          <label class="flex flex-col gap-1 text-[12px] text-muted">
            {reader_goals_minutes_label(undefined, msg)}
            <input
              type="number"
              min="1"
              max="600"
              step="1"
              bind:value={customMinutes}
              class="rounded-md border border-border-strong bg-surface px-2 py-1.5 text-sm text-foreground"
            />
          </label>
        {/if}
        <label class="flex flex-col gap-1 text-[12px] text-muted">
          {reader_goals_start_label(undefined, msg)}
          <input type="date" bind:value={customStart} class="rounded-md border border-border-strong bg-surface px-2 py-1.5 text-sm text-foreground" />
        </label>
        <label class="flex flex-col gap-1 text-[12px] text-muted">
          {reader_goals_end_label(undefined, msg)}
          <input type="date" bind:value={customEnd} class="rounded-md border border-border-strong bg-surface px-2 py-1.5 text-sm text-foreground" />
        </label>
        <div class="flex flex-wrap items-center gap-3 sm:col-span-2">
          <button
            type="submit"
            disabled={customProblem !== null}
            class="rounded-md border border-border-strong px-3 py-1.5 text-sm text-foreground hover:bg-muted-bg disabled:opacity-50"
          >
            {reader_goals_create(undefined, msg)}
          </button>
          {#if customProblem !== null}
            <span class="text-[12px] text-muted">{errorText(customProblem)}</span>
          {/if}
        </div>
      </form>

      {#if error !== null}
        <p class="mt-2 text-[12px] text-danger" role="alert">{errorText(error)}</p>
      {/if}
    </div>
  {/if}
</section>
