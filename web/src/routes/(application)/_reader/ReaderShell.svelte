<script lang="ts">
  import { onMount, type Snippet } from "svelte";
  import { page } from "$app/state";
  import { SidebarProvider, SidebarInset, SidebarTrigger } from "$lib/components/ui/sidebar";
  import { Container } from "$lib/components";
  import { getReaderUiCopy } from "$lib/i18n/reader-copy";
  import { translationIdFromSegments } from "$lib/data/quran";
  import { reader } from "$lib/stores/reader.svelte";
  import { indopakFont } from "$lib/quran/view/indopak-font.svelte";
  import { stickyNav } from "$lib/stores/sticky-nav.svelte";
  import AppSidebar from "./Sidebar.svelte";
  import { changeTypography } from "./typography-change";
  import PositionIndicator from "./PositionIndicator.svelte";
  import TranslationButton from "./TranslationButton.svelte";

  let {
    header,
    position = null,
    children,
  }: {
    header: Snippet;
    /** Route-known mushaf position for the sticky bar's live indicator; absent on non-content routes. */
    position?: { globalPage: number; juz?: number | null; hizb?: number | null } | null;
    children: Snippet;
  } = $props();
  let mounted = $state(false);
  const copy = getReaderUiCopy();

  let headerTop = $derived(stickyNav.collapsed ? "0px" : `${stickyNav.height}px`);

  const primaryId = $derived(
    page.params.lang && page.params.translator
      ? translationIdFromSegments(page.params.lang, page.params.translator)
      : null,
  );

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
      class="sticky z-10 border-b border-border bg-background/85 backdrop-blur-xl transition-[top] duration-200 ease-out"
    >
      <div class="mx-auto flex h-14 w-full max-w-[1200px] items-center gap-3 px-6">
        {#if mounted}
          <SidebarTrigger aria-label={copy.nav.sidebarToggle} title={copy.nav.sidebarToggle} />
          <TranslationButton {primaryId} />
        {/if}
        {@render header()}
        {#if position}
          <PositionIndicator initial={position} />
        {/if}
        {#if mounted}
          <div
            class="ms-auto flex flex-none items-center gap-0.5 rounded-md border border-border p-0.5"
            role="group"
            aria-label={copy.shell.arabicTextSizeLabel}
          >
            <button
              type="button"
              onclick={() => changeTypography(() => reader.smaller())}
              aria-label={copy.shell.smallerArabicTextLabel}
              class="flex h-[34px] w-8 items-center justify-center rounded-sm text-[13px] text-foreground transition-colors hover:bg-surface-hover sm:w-9"
            >
              A&minus;
            </button>
            <span class="h-4 w-px bg-border" aria-hidden="true"></span>
            <button
              type="button"
              onclick={() => changeTypography(() => reader.bigger())}
              aria-label={copy.shell.largerArabicTextLabel}
              class="flex h-[34px] w-8 items-center justify-center rounded-sm text-[15px] text-foreground transition-colors hover:bg-surface-hover sm:w-9"
            >
              A+
            </button>
          </div>
        {/if}
      </div>
    </header>

    <Container class="py-6">
      {#if mounted && reader.arabicScript === "indopak" && indopakFont.status === "loading"}
        <p role="status" class="mb-4 text-sm text-foreground-secondary">{copy.shell.indopakFontLoading}</p>
      {:else if mounted && reader.arabicScript === "indopak" && indopakFont.status === "error"}
        <div role="alert" class="mb-4 flex flex-wrap items-center gap-3 rounded-md border border-border p-4 text-sm">
          <p>{copy.shell.indopakFontUnavailable}</p>
          <button type="button" onclick={() => window.location.reload()} class="rounded-md border border-border px-3 py-2">{copy.shell.reloadPage}</button>
        </div>
      {/if}
      {@render children()}
    </Container>
  </SidebarInset>
</SidebarProvider>
</section>
