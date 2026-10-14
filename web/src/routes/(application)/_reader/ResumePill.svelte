<script lang="ts">
  import { onDestroy } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { Button } from "#lib/components/ui/button/index.js";
  import { loadQuranData } from "#lib/data/quran-data-client.js";
  import { routeContextFromParams } from "#lib/data/quran.js";
  import { getReaderUiCopy } from "#lib/i18n/reader-copy.js";
  import { publicHref } from "#lib/i18n/public-href.js";
  import { readerHrefFor } from "#lib/i18n/reader.js";
  import {
    reader_threads_close,
    reader_threads_resumed,
    reader_threads_start_over,
  } from "#lib/i18n/m/reader.js";
  import { threadPath } from "#lib/reader/thread-path.js";
  import { sessions } from "#lib/sessions/store.svelte.js";

  // Shown after a whole-surah (or whole-page) entry landed on a thread that
  // was already under way: the reader jumped to where it was left, and this
  // one quiet line offers the other choice. Fades on its own.
  const AUTO_HIDE_MS = 9000;
  const copy = getReaderUiCopy();
  let timer: ReturnType<typeof setTimeout> | null = null;

  const offer = $derived(sessions.offer);

  $effect(() => {
    if (offer === null) return;
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => sessions.dismissOffer(), AUTO_HIDE_MS);
  });

  onDestroy(() => {
    if (timer) clearTimeout(timer);
  });

  async function startOver(): Promise<void> {
    const draft = sessions.startOver();
    if (draft === null) return;
    const quranData = await loadQuranData();
    const path = threadPath(draft, routeContextFromParams(page.params), quranData);
    if (path !== null) {
      await goto(publicHref(readerHrefFor(copy.locale, path)), { replace: true });
    }
    window.scrollTo({ top: 0 });
  }
</script>

{#if offer !== null}
  <div
    role="status"
    class="sticky top-16 z-10 mx-auto my-2 flex w-fit max-w-full items-center gap-2 rounded-pill border border-border bg-popover py-1 ps-4 pe-1 text-[13px] text-foreground-secondary shadow-sm"
  >
    <span class="truncate">{reader_threads_resumed()}</span>
    <Button size="sm" variant="secondary" onclick={startOver}>{reader_threads_start_over()}</Button>
    <Button size="sm" variant="ghost" onclick={() => sessions.dismissOffer()}>
      {reader_threads_close()}
    </Button>
  </div>
{/if}
