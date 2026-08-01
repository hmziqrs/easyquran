<script lang="ts">
  import { onMount } from "svelte";
  import { Nav } from "$lib/components/nav";
  import { Tweaks } from "$lib/components/tweaks";
  import { reader } from "$lib/stores/reader.svelte";

  let { children } = $props();

  onMount(() => {
    reader.hydrate();
  });
</script>

<div class="flex min-h-screen flex-col">
  <Nav />
  <main id="main" tabindex="-1" class="flex-1 pb-28">{@render children()}</main>
</div>
<Tweaks />
