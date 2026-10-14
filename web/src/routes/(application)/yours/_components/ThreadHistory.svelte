<script lang="ts">
  import { onMount } from "svelte";
  import { Icon } from "#lib/components/icon/index.js";
  import { Badge } from "#lib/components/ui/badge/index.js";
  import { Button } from "#lib/components/ui/button/index.js";
  import * as DropdownMenu from "#lib/components/ui/dropdown-menu/index.js";
  import { iconButtonVariants } from "#lib/components/ui/icon-button/index.js";
  import type { QuranData } from "#lib/data/quran-data.js";
  import type { SurahRouteContext } from "#lib/data/quran.js";
  import type { UiLocale } from "#lib/i18n/locales.js";
  import {
    reader_threads_completed,
    reader_threads_delete,
    reader_threads_history_empty,
    reader_threads_paused,
    reader_threads_show_more,
  } from "#lib/i18n/m/reader.js";
  import { timeAgo } from "#lib/i18n/relative-time.js";
  import { resumeThread } from "#lib/reader/resume.js";
  import type { ReadingThread } from "#lib/sessions/schema.js";
  import { sessions } from "#lib/sessions/store.svelte.js";

  // History: finished, restarted, removed and paused (30 days untouched)
  // threads — kept, never shown in Continue. Tapping one reopens it.
  let {
    quranData,
    ctx,
    locale,
    limit,
  }: {
    quranData: QuranData | undefined;
    ctx: SurahRouteContext;
    locale: UiLocale;
    limit: number;
  } = $props();

  const msg = $derived({ locale });
  let expanded = $state(false);

  const archive = $derived(sessions.archive);
  const items = $derived(expanded ? archive.items : archive.items.slice(0, limit));
  const canShowMore = $derived(
    (!expanded && archive.items.length > limit) || (expanded && archive.next !== null),
  );

  function status(thread: ReadingThread): string {
    if (thread.endReason === "completed") return reader_threads_completed(undefined, msg);
    return reader_threads_paused(undefined, msg);
  }

  function showMore(): void {
    if (!expanded) {
      expanded = true;
      return;
    }
    void sessions.loadArchive();
  }

  onMount(() => {
    void sessions.loadArchive(true);
  });
</script>

{#if archive.loaded && archive.items.length === 0}
  <p class="mt-2 text-[13px] text-muted">{reader_threads_history_empty(undefined, msg)}</p>
{:else if items.length > 0}
  <ul class="mt-2 divide-y divide-border overflow-hidden rounded-md border border-border bg-surface">
    {#each items as thread (thread.id)}
      <li class="flex items-center gap-3 px-4 py-2.5">
        <button type="button" class="min-w-0 flex-1 text-start" onclick={() => resumeThread(thread, ctx)}>
          <span class="block truncate text-sm font-medium text-foreground">
            {quranData?.surahByNum(thread.surah)?.name ?? `Surah ${thread.surah}`}
            <span class="font-normal text-muted">{thread.surah}:{thread.ayah}</span>
          </span>
          <span class="text-[12.5px] text-muted">{timeAgo(thread.updatedAt, locale)}</span>
        </button>
        <Badge variant={thread.endReason === "completed" ? "default" : "secondary"}>{status(thread)}</Badge>
        <DropdownMenu.Root>
          <DropdownMenu.Trigger class={iconButtonVariants({ variant: "ghost", size: "sm" })}>
            <Icon name="dots" size={16} />
            <span class="sr-only">{reader_threads_delete(undefined, msg)}</span>
          </DropdownMenu.Trigger>
          <DropdownMenu.Content align="end" sideOffset={4}>
            <DropdownMenu.Group>
              <DropdownMenu.Item variant="destructive" onSelect={() => sessions.remove(thread.id)}>
                {reader_threads_delete(undefined, msg)}
              </DropdownMenu.Item>
            </DropdownMenu.Group>
          </DropdownMenu.Content>
        </DropdownMenu.Root>
      </li>
    {/each}
  </ul>
  {#if canShowMore}
    <div class="mt-2">
      <Button variant="secondary" size="sm" disabled={archive.loading} onclick={showMore}>
        {reader_threads_show_more(undefined, msg)}
      </Button>
    </div>
  {/if}
{/if}
