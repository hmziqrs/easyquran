<!--
  Player — the sticky bottom recitation bar, only mounted while
  reader.isPlaying. Play/pause circle (Icon play when paused or at end, else
  pause), now-playing ref + reciter, a gold progress bar (reader.progressPct —
  the documented dynamic width exception), the time label, and a close button.
-->
<script lang="ts">
  import { onDestroy } from "svelte";
  import { reader } from "$lib/stores/reader.svelte";
  import { Icon } from "$lib/components/icon";

  const showPlay = $derived(reader.isPaused || reader.atEnd);

  // Stop the simulated recitation when the reader unmounts (route change) so
  // the interval doesn't keep mutating state in the background.
  onDestroy(() => reader.stop());
</script>

{#if reader.isPlaying}
  <div
    class="fixed inset-x-0 bottom-0 z-50 border-t border-line bg-bg-3 shadow-[0_-12px_30px_-20px_rgba(0,0,0,0.35)] dark:shadow-[0_-12px_30px_-20px_rgba(0,0,0,0.55)]"
  >
    <div class="mx-auto flex max-w-[1320px] items-center gap-[18px] px-5 py-3.5 sm:px-7">
      <button
        type="button"
        onclick={() => reader.togglePlay()}
        aria-label={showPlay ? "Play" : "Pause"}
        class="flex-none flex h-[38px] w-[38px] items-center justify-center rounded-full bg-accent text-accent-fg transition-[filter] duration-150 hover:brightness-110"
      >
        <Icon name={showPlay ? "play" : "pause"} size={14} />
      </button>

      <div class="flex-none flex w-[180px] flex-col gap-0.5">
        <span class="text-[13.5px] font-medium text-fg">{reader.nowPlayingRef}</span>
        <span class="text-xs text-fg-3">{reader.reciter}</span>
      </div>

      <div class="h-1 flex-1 overflow-hidden rounded-full bg-line-2">
        <div class="h-full rounded-full bg-pop" style="width:{reader.progressPct}"></div>
      </div>

      <span class="flex-none w-[78px] text-right text-xs text-fg-3">{reader.timeLabel}</span>

      <button
        type="button"
        onclick={() => reader.stop()}
        aria-label="Close player"
        class="flex-none text-fg-3 transition-colors hover:text-fg"
      >
        <Icon name="x" size={17} />
      </button>
    </div>
  </div>
{/if}
