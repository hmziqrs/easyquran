<script lang="ts">
  import { update } from "$lib/offline/update.svelte";

  let dismissed = $state(false);

  const visible = $derived(update.available && !dismissed);

  $effect(() => {
    if (!update.available) dismissed = false;
  });

  function reload(): void {
    update.apply();
  }
  function dismiss(): void {
    dismissed = true;
  }
</script>

{#if visible}
  <div
    role="status"
    aria-live="polite"
    class="fixed left-1/2 top-4 z-[1002] flex w-[min(92vw,380px)] -translate-x-1/2 items-start gap-3 rounded-xl border border-line-2 bg-bg-1/95 p-3.5 shadow-[0_18px_40px_rgba(0,0,0,0.4)] backdrop-blur"
  >
    <div class="flex size-8 shrink-0 items-center justify-center rounded-lg bg-accent-soft text-accent">
      <span class="text-base leading-none" aria-hidden="true">↑</span>
    </div>
    <div class="min-w-0 flex-1">
      <p class="text-sm font-medium text-fg">A new version is ready</p>
      <p class="mt-0.5 text-xs leading-relaxed text-fg-2">Reload to update every open tab.</p>
      <button
        type="button"
        onclick={reload}
        class="mt-1.5 inline-flex items-center rounded-md bg-accent px-2.5 py-1 text-xs font-medium text-accent-fg transition-[filter] duration-150 hover:brightness-[0.96]"
      >
        Reload open tabs
      </button>
    </div>
    <button
      type="button"
      onclick={dismiss}
      aria-label="Dismiss update notification"
      class="shrink-0 text-fg-3 transition-colors hover:text-fg">✕</button
    >
  </div>
{/if}
