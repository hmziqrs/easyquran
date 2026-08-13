<script lang="ts">
  import { getReaderUiCopy } from "$lib/i18n/reader-copy";

  let {
    loadFailed,
    workerDegraded,
    apiDegraded,
    quranStatusError,
    initialEmpty,
    failedPage,
    onRetry,
  }: {
    loadFailed: boolean;
    workerDegraded: boolean;
    apiDegraded: boolean;
    quranStatusError: boolean;
    initialEmpty: boolean;
    failedPage: number | null;
    onRetry: () => void;
  } = $props();

  const copy = getReaderUiCopy();
</script>

<div
  role="status"
  class="flex items-center justify-between gap-3 border-t border-line bg-bg-2 px-5 py-3 text-sm text-fg-2 sm:px-9"
>
  <span>
    {#if loadFailed && initialEmpty}
      {copy.shell.translationUnavailable}
    {:else if loadFailed && failedPage !== null}
      {copy.shell.pageUnavailable(failedPage)}
    {:else if workerDegraded}
      {copy.shell.offlineCopyUnavailable}
    {:else if apiDegraded}
      {copy.shell.networkUnavailable}
    {:else if quranStatusError}
      {copy.shell.offlineDataUnavailable}
    {:else}
      {copy.shell.moreAyahsUnavailable}
    {/if}
  </span>
  {#if loadFailed && (initialEmpty || failedPage !== null)}
    <button
      type="button"
      onclick={onRetry}
      aria-label={copy.shell.retry}
      class="shrink-0 rounded-full border border-line px-3 py-1 text-xs text-fg transition-colors hover:bg-bg-1"
    >
      {copy.shell.retry}
    </button>
  {/if}
</div>
