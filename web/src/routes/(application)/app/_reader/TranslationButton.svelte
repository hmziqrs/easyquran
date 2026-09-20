<script lang="ts">
  import { stackedTranslations } from "$lib/stores/stacked-translations.svelte";
  import { reader } from "$lib/stores/reader.svelte";
  import { getReaderUiCopy } from "$lib/i18n/reader-copy";
  import TranslationModal from "./TranslationModal.svelte";

  let { primaryId = null }: { primaryId?: string | null } = $props();

  const copy = getReaderUiCopy();
  let open = $state(false);

  const hiddenCount = $derived(
    stackedTranslations.ids.filter((id) => id !== primaryId).length,
  );

  // Registered asynchronously so `@tanstack/hotkeys` stays out of the initial
  // bundle — same shape as the app layout's Mod+, hotkey. "t" opens the
  // translation modal.
  $effect(() => {
    let destroyed = false;
    let cleanup: (() => void) | undefined;

    void import("$lib/hotkeys.svelte").then(({ registerHotkey }) => {
      if (destroyed) return;
      const translationsHotkey = registerHotkey("T", (event) => {
        // IME: don't let a composition session's chord hijack the page.
        if (event.isComposing) return;
        open = true;
      }, { meta: { name: "Open translations" } });
      cleanup = () => {
        translationsHotkey.unregister();
      };
    });

    return () => {
      destroyed = true;
      cleanup?.();
    };
  });
</script>

<div class="flex-none">
  <button
    type="button"
    onclick={() => (open = true)}
    aria-haspopup="dialog"
    aria-expanded={open}
    aria-label={copy.translations.open}
    title={copy.translations.open}
    class="relative flex h-9 w-9 items-center justify-center rounded-md border border-border bg-background-subtle text-foreground-secondary transition-colors hover:border-border-strong hover:text-foreground"
  >
    <!-- U9: Google-Translate-style mark (rounded square, 文 + A with swap
         arrows). Static brand blue on purpose — the user asked for that logo
         look; the button chrome (border/hover) stays theme-aware. -->
    <svg
      width="17"
      height="17"
      viewBox="0 0 24 24"
      aria-hidden="true"
      class="flex-none"
    >
      <rect width="24" height="24" rx="5.5" fill="#1a73e8" />
      <g fill="none" stroke="#ffffff" stroke-linecap="round" stroke-linejoin="round">
        <!-- 文 -->
        <path d="M5.9 6.8h7.4" stroke-width="1.5" />
        <path d="M9.6 6.8v4.7" stroke-width="1.5" />
        <path d="m9.6 9.3-3 4.1M9.6 9.3l3 4.1" stroke-width="1.5" />
        <!-- A -->
        <path d="m13.3 17.9 1.9-3.8 1.9 3.8" stroke-width="1.3" />
        <path d="M14 16.6h2.4" stroke-width="1.3" />
        <!-- swap arrows around the A -->
        <path d="M12.4 12.5c1-.7 2.3-.9 3.5-.4" stroke-width="1.1" />
        <path d="m15.4 11.3 1 .3-.3 1" stroke-width="1.1" />
        <path d="M18.9 15.4c-.2 1.2-1 2.2-2.1 2.7" stroke-width="1.1" />
        <path d="m16.6 18.4-.9-.4.4-1" stroke-width="1.1" />
      </g>
    </svg>
    {#if hiddenCount > 0}
      <span
        class="absolute -end-1.5 -top-1.5 flex h-[17px] min-w-[17px] items-center justify-center rounded-pill bg-foreground px-1 text-[10px] font-medium text-background"
        title={reader.isReadingMode ? copy.translations.hiddenTip : undefined}
      >
        {hiddenCount}
      </span>
    {/if}
  </button>

  <TranslationModal bind:open {primaryId} />
</div>
