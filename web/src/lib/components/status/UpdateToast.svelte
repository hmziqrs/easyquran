<script lang="ts">
  import { update } from "$lib/offline/update.svelte";
  import { getLocale } from "$lib/paraglide/runtime.js";
  import { reader_dismiss_update, reader_new_version_ready, reader_reload_open_tabs, reader_reload_update } from "$lib/i18n/m/reader";
  import type { UiLocale } from "$lib/i18n/locales";

  // Locale changes navigate to a localized URL (full reload), so resolving once at init is sound — same idiom as the search palette.
  // SAFETY: paraglide is compiled for exactly the UI locales (en/ar in messages/), so getLocale() only ever returns a UiLocale at runtime.
  const locale = getLocale() as UiLocale;
  const options = { locale } as const;

  const copy = {
    heading: reader_new_version_ready(undefined, options),
    note: reader_reload_update(undefined, options),
    reload: reader_reload_open_tabs(undefined, options),
    dismiss: reader_dismiss_update(undefined, options),
  };

  // Dismissal is the store's per-waiting-version record (I3): a new waiting
  // worker resets it, so the banner re-prompts only for genuinely new versions.
  const visible = $derived(update.available && !update.dismissed);

  function reload(): void {
    update.apply();
  }
  function dismiss(): void {
    update.dismiss();
  }
</script>

{#if visible}
  <div
    role="status"
    aria-live="polite"
    class="fixed left-1/2 top-4 z-[1002] flex w-[min(92vw,380px)] -translate-x-1/2 items-start gap-3 rounded-xl border border-border bg-surface/95 p-3.5 backdrop-blur"
  >
    <div class="flex size-8 shrink-0 items-center justify-center rounded-sm bg-primary-soft text-primary">
      <span class="text-base leading-none" aria-hidden="true">↑</span>
    </div>
    <div class="min-w-0 flex-1">
      <p class="text-body-s font-medium text-foreground">{copy.heading}</p>
      <p class="mt-0.5 text-caption leading-relaxed text-foreground-secondary">{copy.note}</p>
      <button
        type="button"
        onclick={reload}
        class="mt-1.5 inline-flex items-center rounded-pill bg-primary px-2.5 py-1 text-caption font-medium text-primary-foreground transition-[filter] duration-150 hover:brightness-[0.96]"
      >
        {copy.reload}
      </button>
    </div>
    <button
      type="button"
      onclick={dismiss}
      aria-label={copy.dismiss}
      class="shrink-0 rounded-pill px-1 text-muted transition-colors hover:text-foreground">✕</button
    >
  </div>
{/if}
