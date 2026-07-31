<!--
  App shell — the chrome for the reader at /app. Shares the marketing <Nav/>
  (so the header is identical across the site) and the same floating theme
  tweaker, but carries no footer. The reader sidebar now owns tab switching, so the old
  APP_PAGES tab bar is gone. <main> gets bottom padding so the sticky player
  never covers the last verse.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { Nav } from "$lib/components/nav";
  import { Tweaks } from "$lib/components/tweaks";
  import { reader } from "$lib/stores/reader.svelte";

  let { children } = $props();

  // Pull saved reader state in AFTER mount so the prerendered HTML (built from
  // DEFAULTS) matches the first client render — no hydration mismatch. (The
  // offline engine is booted from the root layout, so it starts on the landing
  // page too — see routes/+layout.svelte.)
  onMount(() => {
    reader.hydrate();
  });
</script>

<div class="flex min-h-screen flex-col">
  <Nav />
  <main id="main" tabindex="-1" class="flex-1 pb-28">{@render children()}</main>
</div>
<!-- The theme tweaker rides along in the reader too: palette choices are global
     (they rewrite the tokens on <html>), so they must be adjustable from the
     surface people actually spend their time reading on. -->
<Tweaks />
