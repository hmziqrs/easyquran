<script lang="ts">
  import { onMount, type Snippet } from "svelte";
  import { page } from "$app/state";
  import { SidebarProvider, SidebarInset, SidebarTrigger } from "$lib/components/ui/sidebar";
  import { Container } from "$lib/components";
  import { getReaderUiCopy } from "$lib/i18n/reader-copy";
  import { translationIdFromSegments } from "$lib/data/quran";
  import { reader } from "$lib/stores/reader.svelte";
  import { stickyNav } from "$lib/stores/sticky-nav.svelte";
  import AppSidebar from "./Sidebar.svelte";
  import { changeTypography } from "./typography-change";
  import TranslationButton from "./TranslationButton.svelte";

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
      <!-- One frame with the site nav above and the page column below: 1200px, 24px gutter,
           56px tall like the nav, every control 40px. -->
      <div class="mx-auto flex h-14 w-full max-w-[1200px] items-center gap-3 px-6">
        {#if mounted}
          <SidebarTrigger aria-label={copy.nav.sidebarToggle} title={copy.nav.sidebarToggle} />
          <TranslationButton {primaryId} />
        {/if}
        {@render header()}
        {#if mounted}
          <!-- Arabic size lives in the sticky bar so it is always in reach (user pick); the
               reader's typography effect keeps the reading position across the change. -->
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
      {@render children()}
    </Container>
  </SidebarInset>
</SidebarProvider>
</section>
