<script lang="ts">
  import { onMount, type Snippet } from "svelte";
  import { page } from "$app/state";
  import { SidebarProvider, SidebarInset, SidebarTrigger } from "$lib/components/ui/sidebar";
  import { Container } from "$lib/components";
  import { Icon } from "$lib/components/icon";
  import { getReaderUiCopy } from "$lib/i18n/reader-copy";
  import { translationIdFromSegments } from "$lib/data/quran";
  import { reader } from "$lib/stores/reader.svelte";
  import { stackedTranslations } from "$lib/stores/stacked-translations.svelte";
  import { stickyNav } from "$lib/stores/sticky-nav.svelte";
  import AppSidebar from "./Sidebar.svelte";
  import TranslationButton from "./TranslationButton.svelte";
  import { readingBannerVisible, readingModeUi } from "./reading-mode-guard.svelte";

  let { header, children }: { header: Snippet; children: Snippet } = $props();
  let mounted = $state(false);
  const copy = getReaderUiCopy();

  let headerTop = $derived(stickyNav.collapsed ? "0px" : `${stickyNav.height}px`);

  // Route-primary translation id, derived exactly like the app layout's
  // worker-pinning effect does from the /t/[lang]/[translator] params.
  const primaryId = $derived(
    page.params.lang && page.params.translator
      ? translationIdFromSegments(page.params.lang, page.params.translator)
      : null,
  );

  // Mode-switch banner: URL-initiated fallback ONLY (?mode=reading loads with
  // no gesture to confirm against). UI-initiated switches get the upfront
  // ReadingModeDialog confirmation instead (reading-mode-guard), so both must
  // never show together. One-shot per transition — dismissing sticks until the
  // reader leaves reading mode or the selection changes. The store itself is
  // never touched by mode switches.
  let bannerDismissed = $state(false);
  const hiddenCount = $derived(
    stackedTranslations.ids.filter((id) => id !== primaryId).length,
  );
  const bannerVisible = $derived(
    readingBannerVisible({
      reading: reader.isReadingMode,
      hiddenCount,
      dismissed: bannerDismissed,
    }),
  );
  let prevReading = false;
  let prevHidden = -1;
  $effect(() => {
    const reading = reader.isReadingMode;
    const hidden = hiddenCount;
    const leftReading = prevReading && !reading;
    const selectionsChanged = prevHidden >= 0 && hidden !== prevHidden;
    if (leftReading) readingModeUi.reset();
    if ((leftReading || selectionsChanged) && bannerDismissed) bannerDismissed = false;
    prevReading = reading;
    prevHidden = hidden;
  });

  function dismissBanner(): void {
    bannerDismissed = true;
  }

  onMount(() => {
    mounted = true;
  });
</script>

<section lang={copy.locale} dir={copy.direction} data-reader-shell>
<SidebarProvider open={false}>
  {#if mounted}
    <AppSidebar />
  {/if}

  <SidebarInset>
    <header
      style:top={headerTop}
      class="sticky z-10 min-h-11 border-b border-border bg-background/80 py-2 backdrop-blur-xl transition-[top] duration-200 ease-out"
    >
      <div class="flex w-full items-center gap-3 px-5 sm:px-7 lg:px-10">
        {#if mounted}
          <SidebarTrigger aria-label={copy.nav.sidebarToggle} title={copy.nav.sidebarToggle} />
          <TranslationButton {primaryId} />
        {/if}
        {@render header()}
      </div>
    </header>

    {#if bannerVisible}
      <div
        role="status"
        aria-live="polite"
        class="mx-5 mt-3 flex items-center justify-between gap-3 rounded-md border border-border bg-background-subtle px-3 py-2 text-[12.5px] text-foreground-secondary sm:mx-7 lg:mx-10"
      >
        <span>{copy.translations.readingBanner}</span>
        <button
          type="button"
          onclick={dismissBanner}
          aria-label={copy.translations.dismissBanner}
          class="flex h-7 w-7 flex-none items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-surface-hover hover:text-foreground"
        >
          <Icon name="x" size={13} />
        </button>
      </div>
    {/if}

    <Container class="max-w-[1180px] py-6">
      {@render children()}
    </Container>
  </SidebarInset>
</SidebarProvider>
</section>
