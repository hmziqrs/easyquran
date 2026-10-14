<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";

  import { loadQuranData, peekQuranData } from "#lib/data/quran-data-client.js";
  import type { QuranData } from "#lib/data/quran-data.js";
  import { getReaderUiCopy } from "#lib/i18n/reader-copy.js";
  import { appLocale } from "#lib/i18n/app-locale.js";
  import { publicHref } from "#lib/i18n/public-href.js";
  import { yoursPageHref } from "#lib/i18n/reader.js";
  import { peekTranslationName } from "#lib/quran/catalogue.js";
  import {
    reader_cards_activity,
    reader_cards_back,
    reader_cards_contents,
    reader_cards_duplicate,
    reader_cards_edit_title,
    reader_cards_full,
    reader_cards_last_recited,
    reader_cards_never,
    reader_cards_not_found,
    reader_cards_plus_one,
    reader_cards_remove,
    reader_cards_reset,
    reader_cards_schedule,
    reader_cards_schedule_empty,
    reader_cards_streak_best,
    reader_cards_streak_current,
    reader_cards_streak_day,
    reader_cards_streak_days,
    reader_cards_stats_month,
    reader_cards_stats_today,
    reader_cards_stats_total,
    reader_cards_stats_week,
    reader_cards_target_done,
    reader_cards_translation_arabic,
  } from "#lib/i18n/m/reader.js";
  import { Icon, Seo } from "#lib/components/index.js";
  import { Button } from "#lib/components/ui/button/index.js";
  import { Skeleton } from "#lib/components/ui/skeleton/index.js";
  import { homeCards, type HomeCard } from "#lib/home/cards.svelte.js";
  import {
    cardContents,
    cardEntryAttrs,
    scheduleBadgeCopy,
    tileCopyFor,
    yoursBackHref,
    type TileCopy,
  } from "#lib/home/card-copy.js";
  import { cardStats, type CardStats } from "#lib/home/card-stats.js";
  import CardForm from "#lib/home/CardForm.svelte";
  import { armCardUndo } from "#lib/home/card-undo.svelte.js";
  import { slotStateFor } from "#lib/home/schedule.js";

  /**
   * Per-card detail: statistics from the recite-event ring, contents links,
   * the read-only schedule and the edit/delete surface. Client-only like
   * /yours — the card store is device-local (§11.3), so nothing personal
   * renders in SSR HTML: pre-mount shows skeletons and the not-found branch
   * sits behind the mounted gate (before hydrate() the list is empty by
   * design, so an unknown id can never flash).
   */

  const locale = appLocale();
  const msgOptions = { locale };
  const copy = getReaderUiCopy(locale);

  const h = {
    back: reader_cards_back(undefined, msgOptions),
    notFound: reader_cards_not_found(undefined, msgOptions),
    contents: reader_cards_contents(undefined, msgOptions),
    schedule: reader_cards_schedule(undefined, msgOptions),
    scheduleEmpty: reader_cards_schedule_empty(undefined, msgOptions),
    edit: reader_cards_edit_title(undefined, msgOptions),
    duplicate: reader_cards_duplicate(undefined, msgOptions),
    reset: reader_cards_reset(undefined, msgOptions),
    remove: reader_cards_remove(undefined, msgOptions),
    full: reader_cards_full(undefined, msgOptions),
    recite: reader_cards_plus_one(undefined, msgOptions),
    targetDone: reader_cards_target_done(undefined, msgOptions),
    activity: reader_cards_activity(undefined, msgOptions),
    never: reader_cards_never(undefined, msgOptions),
    lastRecited: reader_cards_last_recited(undefined, msgOptions),
    translationArabic: reader_cards_translation_arabic(undefined, msgOptions),
    stats: {
      total: reader_cards_stats_total(undefined, msgOptions),
      today: reader_cards_stats_today(undefined, msgOptions),
      week: reader_cards_stats_week(undefined, msgOptions),
      month: reader_cards_stats_month(undefined, msgOptions),
    },
    streaks: {
      current: reader_cards_streak_current(undefined, msgOptions),
      best: reader_cards_streak_best(undefined, msgOptions),
    },
  };

  const cardId = $derived(page.params.id ?? "");

  let mounted = $state(false);
  let quranData = $state<QuranData | undefined>(peekQuranData());

  const card = $derived(mounted ? homeCards.list.find((entry) => entry.id === cardId) : undefined);
  const view = $derived.by<TileCopy | null>(() => {
    const current = card;
    return current === undefined ? null : tileCopyFor(current, quranData, locale);
  });
  // Recomputes on every store mutation of this card (increment/replace create
  // a new card object); Date.now() pins to that render pass.
  const stats = $derived.by<CardStats | null>(() => {
    const current = card;
    return current === undefined ? null : cardStats(current, Date.now());
  });
  const rows = $derived.by<ReturnType<typeof cardContents>>(() => {
    const current = card;
    return current === undefined ? [] : cardContents(current, quranData, locale);
  });
  const scheduleSlots = $derived(card?.schedule ?? []);
  const done = $derived(card !== undefined && card.target !== undefined && card.count >= card.target);

  interface BadgeCopy {
    readonly due: boolean;
    readonly label: string;
  }

  /* Same precedence as the tile: a finished card is never nagged; otherwise
     the schedule's due/next state (passive hint — never writes count/events). */
  const badge = $derived.by<BadgeCopy | null>(() => {
    if (card === undefined || done) return null;
    const schedule = card.schedule;
    if (schedule === undefined || schedule.length === 0) return null;
    const state = slotStateFor(schedule, Date.now());
    return state === null ? null : scheduleBadgeCopy(state, locale);
  });

  /** Reader-ref rows need surah names — skeleton until quranData lands. */
  function needsQuranData(current: HomeCard): boolean {
    if (current.kind === "surah" || current.kind === "ayah" || current.kind === "range") return true;
    if (current.kind === "collection") return current.items.some((item) => !("slug" in item));
    return false;
  }
  const contentsLoading = $derived(
    card !== undefined && quranData === undefined && needsQuranData(card),
  );

  /** Collections pin per item; every other kind carries one card-level id. */
  const translationLabel = $derived.by<string>(() => {
    const current = card;
    if (current === undefined) return "";
    let sourceId: string | null = null;
    if (current.kind === "collection") sourceId = current.items[0]?.sourceId ?? null;
    else if (current.kind !== "zikr") sourceId = current.sourceId;
    const name = sourceId === null ? null : peekTranslationName(sourceId);
    if (name === null) return h.translationArabic;
    return name;
  });

  function streakDays(count: number): string {
    const message = count === 1 ? reader_cards_streak_day : reader_cards_streak_days;
    return message({ count }, msgOptions);
  }

  function formatDate(ms: number): string {
    return new Intl.DateTimeFormat(locale, { dateStyle: "medium" }).format(new Date(ms));
  }

  function formatStamp(iso: string): string {
    const ms = Date.parse(iso);
    if (Number.isNaN(ms)) return iso;
    return formatDate(ms);
  }

  /* ── Detail-page actions ─────────────────────────────────────────────────── */
  const FORM_ID = "card-edit-form";
  let showForm = $state(false);
  let duplicateFailed = $state(false);
  let editButton = $state<HTMLButtonElement | null>(null);

  // Cancel and successful Save both land here: close + focus back to the opener.
  function closeForm(): void {
    showForm = false;
    editButton?.focus();
  }

  function toggleForm(): void {
    if (showForm) {
      closeForm();
      return;
    }
    duplicateFailed = false;
    showForm = true;
  }

  function recite(): void {
    homeCards.increment(cardId);
  }

  function duplicateCard(): void {
    // Refused (store full): the alert line under the action row says so.
    duplicateFailed = homeCards.duplicate(cardId) === false;
  }

  function resetCount(): void {
    const current = card;
    if (current === undefined || current.count === 0) return;
    homeCards.reset(current.id);
  }

  function removeCard(): void {
    const current = card;
    if (current === undefined) return;
    // House destructive pattern: optimistic remove + 5s undo. The module-owned
    // snapshot arms the /yours banner across this navigation.
    armCardUndo(current);
    homeCards.remove(current.id);
    void goto(yoursBackHref());
  }

  onMount(() => {
    mounted = true;
    homeCards.hydrate();
    if (peekQuranData() === undefined) {
      void loadQuranData().then(
        (data) => {
          quranData = data;
        },
        () => undefined,
      );
    }
  });
</script>

<svelte:head>
  <title>{view === null ? copy.seo.yoursTitle : `${view.title} · EasyQuran`}</title>
</svelte:head>

<!-- noindex: the path carries a device-local uuid and prerender is false, so
     the URL must never enter an index (Seo skips the canonical link here). -->
<Seo
  path={yoursPageHref()}
  title={copy.seo.yoursTitle}
  description={copy.seo.yoursDescription}
  noindex
/>

<div class="mx-auto max-w-[860px] px-5 pt-8 sm:px-7">
  {#if !mounted}
    <!-- Pre-hydrate the store list is empty by design: skeletons, never a
         not-found flash. -->
    <div class="mt-2 flex flex-col gap-2" aria-busy="true">
      <Skeleton class="h-8 w-40" />
      <Skeleton class="h-14 w-full" />
      <div class="grid grid-cols-2 gap-2 sm:grid-cols-4">
        <Skeleton class="h-20 w-full" />
        <Skeleton class="h-20 w-full" />
        <Skeleton class="h-20 w-full" />
        <Skeleton class="h-20 w-full" />
      </div>
      <Skeleton class="h-24 w-full" />
    </div>
  {:else if card === undefined}
    <div class="mt-10 flex flex-col gap-3 rounded-md border border-border bg-surface px-4 py-5">
      <div class="flex items-center gap-3.5">
        <span
          class="flex size-11 flex-none items-center justify-center rounded-sm text-[var(--hue-2-legible)]"
          style:background="var(--hue-2-soft)"
          aria-hidden="true"
        >
          <Icon name="book" size={20} />
        </span>
        <p class="text-[14.5px] leading-relaxed text-foreground-secondary">{h.notFound}</p>
      </div>
      <div>
        <a
          href={yoursBackHref()}
          data-sveltekit-preload-data="hover"
          class="inline-flex h-8 items-center rounded-pill border border-border px-3 text-[12.5px] font-medium text-foreground-secondary transition-colors hover:border-border-strong hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
        >
          {h.back}
        </a>
      </div>
    </div>
  {:else if view !== null && stats !== null}
    <a
      href={yoursBackHref()}
      data-sveltekit-preload-data="hover"
      class="inline-flex h-8 items-center gap-1.5 rounded-pill border border-border px-3 text-[12.5px] font-medium text-foreground-secondary transition-colors hover:border-border-strong hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
    >
      <span aria-hidden="true">←</span>
      {h.back}
    </a>

    <div class="mt-3 flex flex-wrap items-start justify-between gap-3">
      <div class="min-w-0 flex-1">
        <h1 class="text-[28px] font-medium leading-tight tracking-[-0.02em] text-foreground">
          {#if view.arabicTitle}
            <span lang="ar" dir="rtl" class="font-arabic">{view.title}</span>
          {:else}
            {view.title}
          {/if}
        </h1>
        {#if view.meta.length > 0}
          <p class="mt-1 text-[12.5px] text-muted">{view.meta}</p>
        {/if}
      </div>
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

    <div class="mt-4 flex flex-wrap items-center gap-2">
      <Button
        type="button"
        bind:ref={editButton}
        aria-expanded={showForm}
        aria-controls={FORM_ID}
        variant="secondary"
        size="sm"
        class="h-8 px-3 text-[12.5px]"
        onclick={toggleForm}
      >
        {h.edit}
      </Button>
      <Button
        type="button"
        variant="secondary"
        size="sm"
        class="h-8 px-3 text-[12.5px]"
        onclick={duplicateCard}
      >
        {h.duplicate}
      </Button>
      <Button
        type="button"
        variant="secondary"
        size="sm"
        class="h-8 px-3 text-[12.5px]"
        disabled={card.count === 0}
        onclick={resetCount}
      >
        {h.reset}
      </Button>
      <Button
        type="button"
        variant="secondary"
        size="sm"
        class="h-8 px-3 text-[12.5px] text-danger hover:text-danger"
        onclick={removeCard}
      >
        {h.remove}
      </Button>
    </div>
    {#if duplicateFailed}
      <p role="alert" class="mt-2 text-[13px] text-danger">{h.full}</p>
    {/if}

    {#if showForm}
      <!-- The one edit surface: target, translation, schedule, collection
           items — saved through homeCards.replace (count/events untouched). -->
      <div id={FORM_ID} class="mt-3">
        <CardForm quranData={quranData} initial={card} onClose={closeForm} />
      </div>
    {/if}

    <div class="mt-4 grid grid-cols-2 gap-2 sm:grid-cols-4">
      <div class="rounded-md border border-border bg-surface px-3 py-2.5">
        <span class="eyebrow">{h.stats.total}</span>
        <span class="mt-1 block text-xl font-medium text-foreground">{stats.total}</span>
      </div>
      <div class="rounded-md border border-border bg-surface px-3 py-2.5">
        <span class="eyebrow">{h.stats.today}</span>
        <span class="mt-1 block text-xl font-medium text-foreground">{stats.today}</span>
      </div>
      <div class="rounded-md border border-border bg-surface px-3 py-2.5">
        <span class="eyebrow">{h.stats.week}</span>
        <span class="mt-1 block text-xl font-medium text-foreground">{stats.last7}</span>
      </div>
      <div class="rounded-md border border-border bg-surface px-3 py-2.5">
        <span class="eyebrow">{h.stats.month}</span>
        <span class="mt-1 block text-xl font-medium text-foreground">{stats.last30}</span>
      </div>
    </div>
    <div class="mt-2 grid grid-cols-2 gap-2 sm:grid-cols-4">
      <div class="rounded-md border border-border bg-surface px-3 py-2.5">
        <span class="eyebrow">{h.streaks.current}</span>
        <span class="mt-1 block text-xl font-medium text-foreground"
          >{streakDays(stats.currentStreak)}</span
        >
      </div>
      <div class="rounded-md border border-border bg-surface px-3 py-2.5">
        <span class="eyebrow">{h.streaks.best}</span>
        <span class="mt-1 block text-xl font-medium text-foreground"
          >{streakDays(stats.bestStreak)}</span
        >
      </div>
      <div class="rounded-md border border-border bg-surface px-3 py-2.5">
        <span class="eyebrow">{h.lastRecited}</span>
        <span class="mt-1 block text-xl font-medium text-foreground"
          >{stats.lastRecitedAt === null ? h.never : formatDate(stats.lastRecitedAt)}</span
        >
      </div>
    </div>

    <Button
      type="button"
      variant={done ? "primary" : "secondary"}
      class="mt-3 w-full flex-col gap-0.5 py-2"
      aria-label="{h.recite}: {view.title}"
      onclick={recite}
    >
      {#if done}
        <Icon name="check" size={18} />
        <span class="text-[11px] leading-none font-normal">{h.targetDone}</span>
      {:else}
        <span class="text-[15px] leading-none" aria-hidden="true">+</span>
        <span class="text-[11px] leading-none font-normal">{card.count}</span>
      {/if}
    </Button>

    <section class="mt-6">
      <h2 class="eyebrow">{h.activity}</h2>
      <div class="mt-2 flex items-end gap-0.5" role="img" aria-label={h.activity}>
        {#each stats.activity30 as day (day.day)}
          <span
            aria-hidden="true"
            class="h-1.5 flex-1 rounded-pill"
            class:bg-primary={day.count > 0}
            class:bg-surface-hover={day.count === 0}
          ></span>
        {/each}
      </div>
      <p class="sr-only">{h.stats.today}: {stats.today} · {h.stats.month}: {stats.last30}</p>
    </section>

    <section class="mt-6">
      <h2 class="eyebrow">{h.contents}</h2>
      {#if contentsLoading}
        <!-- Surah names resolve only once quranData is in: skeleton keeps the
             section from collapsing mid-load (vanishing-sections rule). -->
        <ul class="mt-2 divide-y divide-border overflow-hidden rounded-md border border-border bg-surface" aria-busy="true">
          <li class="px-4 py-3"><Skeleton class="h-11 w-full" /></li>
          <li class="px-4 py-3"><Skeleton class="h-11 w-full" /></li>
        </ul>
      {:else}
        <ul class="mt-2 divide-y divide-border overflow-hidden rounded-md border border-border bg-surface">
          {#each rows as row, index (index)}
            <li>
              {#if row.href !== null}
                <a
                  {...cardEntryAttrs(card, card.kind === "collection" ? card.items[index] : undefined)}
                  href={publicHref(row.href)}
                  data-sveltekit-preload-data="hover"
                  class="flex items-center gap-2 px-4 py-3 text-sm text-foreground transition-colors hover:bg-surface-hover focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
                >
                  {#if card.kind === "collection"}
                    <span aria-hidden="true" class="flex-none text-[12px] text-muted"
                      >{index + 1}.</span
                    >
                  {/if}
                  <span class="min-w-0 truncate">{row.label}</span>
                </a>
              {:else if card.kind === "zikr"}
                <span
                  lang="ar"
                  dir="rtl"
                  class="block px-4 py-3 text-right font-arabic text-[17px] leading-loose text-foreground"
                  >{row.label}</span
                >
              {:else}
                <span class="flex items-center gap-2 px-4 py-3 text-sm text-foreground">
                  {#if card.kind === "collection"}
                    <span aria-hidden="true" class="flex-none text-[12px] text-muted"
                      >{index + 1}.</span
                    >
                  {/if}
                  <span class="min-w-0 truncate">{row.label}</span>
                </span>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
    </section>

    <section class="mt-6">
      <div class="flex items-center gap-2">
        <h2 class="eyebrow">{h.schedule}</h2>
        {#if badge !== null}
          <span
            class="flex h-6 items-center rounded-pill border px-2 text-[11.5px] font-medium"
            class:border-border-strong={badge.due}
            class:text-primary={badge.due}
            class:border-border={badge.due === false}
            class:text-muted={badge.due === false}
          >
            {badge.label}
          </span>
        {/if}
      </div>
      {#if scheduleSlots.length === 0}
        <p class="mt-2 text-[13px] text-muted">{h.scheduleEmpty}</p>
      {:else}
        <!-- Read-only: the single mutation path is Edit card → ScheduleEditor. -->
        <ul class="mt-2 divide-y divide-border overflow-hidden rounded-md border border-border bg-surface">
          {#each scheduleSlots as slotEntry (slotEntry.id)}
            <li class="flex items-center justify-between gap-3 px-4 py-2.5 text-sm">
              <span class="min-w-0 truncate text-foreground-secondary">{slotEntry.label}</span>
              <span class="flex-none text-foreground">{slotEntry.time}</span>
            </li>
          {/each}
        </ul>
      {/if}
    </section>

    <p class="mt-8 text-[12.5px] text-muted">
      {translationLabel} · {formatStamp(card.createdAt)}
    </p>
  {/if}
</div>
