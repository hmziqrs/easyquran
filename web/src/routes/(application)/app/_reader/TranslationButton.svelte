<script lang="ts">
  import { stackedTranslations } from "$lib/stores/stacked-translations.svelte";
  import { reader } from "$lib/stores/reader.svelte";
  import { getReaderUiCopy } from "$lib/i18n/reader-copy";
  import { Icon } from "$lib/components/icon";
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
    <Icon name="book" size={15} class="text-muted-foreground" />
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
