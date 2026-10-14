<script lang="ts">
  import { Icon } from "#lib/components/icon/index.js";
  import { Button } from "#lib/components/ui/button/index.js";
  import { iconButtonVariants } from "#lib/components/ui/icon-button/index.js";
  import type { QuranData } from "#lib/data/quran-data.js";
  import type { SurahRouteContext } from "#lib/data/quran.js";
  import type { UiLocale } from "#lib/i18n/locales.js";
  import {
    reader_threads_dismiss,
    reader_threads_empty,
    reader_threads_on_desktop,
    reader_threads_on_phone,
    reader_threads_on_tablet,
    reader_threads_sync_error,
    reader_threads_verse_of,
  } from "#lib/i18n/m/reader.js";
  import { timeAgo } from "#lib/i18n/relative-time.js";
  import { resumeThread } from "#lib/reader/resume.js";
  import { deviceKind } from "#lib/sessions/device.js";
  import type { DeviceKind, ReadingThread } from "#lib/sessions/schema.js";
  import { sessions } from "#lib/sessions/store.svelte.js";

  // Continue: every open reading thread, newest first — the first one leads
  // as the big card, the rest are one-tap rows. A thread last touched on
  // another device says so ("on your phone · 5 min ago"): that is the handoff.
  let {
    quranData,
    ctx,
    locale,
    continueLabel,
    limit = 5,
  }: {
    quranData: QuranData | undefined;
    ctx: SurahRouteContext;
    locale: UiLocale;
    /** Lead card CTA ("Continue reading"). */
    continueLabel: string;
    limit?: number;
  } = $props();

  const msg = $derived({ locale });
  const here = deviceKind();
  const DEVICE_LABEL = {
    phone: () => reader_threads_on_phone(undefined, msg),
    tablet: () => reader_threads_on_tablet(undefined, msg),
    desktop: () => reader_threads_on_desktop(undefined, msg),
  } satisfies Record<DeviceKind, () => string>;

  const threads = $derived(sessions.open.slice(0, limit));
  const lead = $derived(threads[0]);
  const rest = $derived(threads.slice(1));

  function where(thread: ReadingThread): string {
    const ago = timeAgo(thread.updatedAt, locale);
    if (thread.device === here) return ago;
    return `${DEVICE_LABEL[thread.device]()} · ${ago}`;
  }

  function surahName(thread: ReadingThread): string {
    return quranData?.surahByNum(thread.surah)?.name ?? `Surah ${thread.surah}`;
  }

  interface ThreadProgress {
    readonly total: number;
    readonly fraction: number;
  }

  function progressOf(thread: ReadingThread): ThreadProgress {
    const total = quranData?.surahByNum(thread.surah)?.ayahCount ?? 0;
    return { total, fraction: total > 0 ? Math.min(1, thread.ayah / total) : 0 };
  }
</script>

{#if sessions.enqueueError !== null}
  <p role="status" class="mt-3 text-[13px] text-danger">{reader_threads_sync_error(undefined, msg)}</p>
{/if}

{#if lead === undefined}
  <div class="mt-2 flex items-center gap-3.5 rounded-md border border-border bg-surface px-4 py-4 sm:px-5">
    <span
      class="flex size-11 flex-none items-center justify-center rounded-sm text-[var(--hue-2-legible)]"
      style:background="var(--hue-2-soft)"
      aria-hidden="true"
    >
      <Icon name="book" size={20} />
    </span>
    <p class="text-[14.5px] leading-relaxed text-foreground-secondary">{reader_threads_empty(undefined, msg)}</p>
  </div>
{:else}
  {@const progress = progressOf(lead)}
  <div class="mt-2 overflow-hidden rounded-md border border-border bg-surface">
    <div class="px-4 py-4 sm:px-5">
      <div class="flex items-start gap-3">
        <div class="min-w-0 flex-1">
          <p class="truncate text-xl font-medium text-foreground">{surahName(lead)}</p>
          <p class="mt-1 text-sm text-foreground-secondary">
            {#if progress.total > 0}
              {reader_threads_verse_of({ ayah: lead.ayah, total: progress.total }, msg)}
            {:else}
              {lead.surah}:{lead.ayah}
            {/if}
            <span class="text-muted">· {where(lead)}</span>
          </p>
        </div>
        <button
          type="button"
          class={iconButtonVariants({ variant: "ghost", size: "sm" })}
          aria-label={reader_threads_dismiss(undefined, msg)}
          title={reader_threads_dismiss(undefined, msg)}
          onclick={() => sessions.dismiss(lead.id)}
        >
          <Icon name="x" size={16} />
        </button>
      </div>
      {#if progress.total > 0}
        <div
          class="mt-4 h-1.5 w-full overflow-hidden rounded-pill bg-surface-hover"
          role="progressbar"
          aria-valuenow={lead.ayah}
          aria-valuemax={progress.total}
        >
          <div class="h-full rounded-pill bg-primary" style:width="{Math.round(progress.fraction * 100)}%"></div>
        </div>
      {/if}
      <div class="mt-4">
        <Button onclick={() => resumeThread(lead, ctx)} arrow>{continueLabel}</Button>
      </div>
    </div>
    {#if rest.length > 0}
      <ul class="divide-y divide-border border-t border-border">
        {#each rest as thread (thread.id)}
          <li class="flex items-center gap-3 px-4 py-2.5 sm:px-5">
            <button
              type="button"
              class="min-w-0 flex-1 text-start"
              onclick={() => resumeThread(thread, ctx)}
            >
              <span class="block truncate text-sm font-medium text-foreground">
                {surahName(thread)} <span class="font-normal text-muted">{thread.surah}:{thread.ayah}</span>
              </span>
              <span class="text-[12.5px] text-muted">{where(thread)}</span>
            </button>
            <button
              type="button"
              class={iconButtonVariants({ variant: "ghost", size: "sm" })}
              aria-label={reader_threads_dismiss(undefined, msg)}
              title={reader_threads_dismiss(undefined, msg)}
              onclick={() => sessions.dismiss(thread.id)}
            >
              <Icon name="x" size={14} />
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
{/if}
