<script lang="ts">
  import { onMount } from "svelte";
  import amiriArabic from "@fontsource/amiri/files/amiri-arabic-400-normal.woff2?url";
  import { Nav } from "$lib/components/nav";
  import { Tweaks } from "$lib/components/tweaks";
  import { reader } from "$lib/stores/reader.svelte";

  let { children } = $props();
  let menuOpen = $state(false);

  onMount(() => {
    reader.hydrate();
    document.documentElement.dataset.readerHydrated = "true";

    const syncMenu = (): void => {
      menuOpen = document.getElementById("site-panel") !== null;
    };
    syncMenu();
    const observer = new MutationObserver(syncMenu);
    observer.observe(document.body, { childList: true, subtree: true });

    return () => observer.disconnect();
  });
</script>

<svelte:head>
  <link rel="preload" href={amiriArabic} as="font" type="font/woff2" crossorigin="anonymous" />
</svelte:head>

<div class="flex min-h-screen flex-col">
  <Nav collapsible />
  <main
    id="main"
    tabindex="-1"
    inert={menuOpen || undefined}
    aria-hidden={menuOpen || undefined}
    class="flex-1 pb-28"
  >{@render children()}</main>
</div>
<Tweaks />
