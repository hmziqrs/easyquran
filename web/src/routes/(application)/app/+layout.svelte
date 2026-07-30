<!--
  App shell — the chrome for the reader at /app. Shares the marketing <Nav/>
  (so the header is identical across the site) but carries no footer and no
  appearance panel. The reader sidebar now owns tab switching, so the old
  APP_PAGES tab bar is gone. <main> gets bottom padding so the sticky player
  never covers the last verse.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { Nav } from "$lib/components/nav";
  import { reader } from "$lib/stores/reader.svelte";
  import { bootOfflineEngine } from "$lib/quran/offline";

  let { children } = $props();

  // Pull saved reader state in AFTER mount so the prerendered HTML (built from
  // DEFAULTS) matches the first client render — no hydration mismatch.
  onMount(() => {
    reader.hydrate();
    // Boot the offline engine in the background: it fetches + verifies the two
    // Arabic DBs from R2 into OPFS and starts the sqlite-wasm Worker. Fully
    // best-effort — the reader paints from prerendered data regardless.
    void bootOfflineEngine();
  });
</script>

<div class="flex min-h-screen flex-col">
  <Nav />
  <main id="main" tabindex="-1" class="flex-1 pb-28">{@render children()}</main>
</div>
