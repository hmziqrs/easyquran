<!--
  /app — the reader. A centered max-w-[1320px] two-column grid: the sticky
  Sidebar (288px) on the left and the reading section on the right. The section
  swaps between search Results (reader.hasQuery) and the SurahReader. The
  sticky Player mounts whenever audio is playing. All state flows through the
  `reader` store; this file only wires the regions together.
-->
<script lang="ts">
  import { Seo } from "$lib/components";
  import { reader } from "$lib/stores/reader.svelte";
  import Sidebar from "./_reader/Sidebar.svelte";
  import SurahReader from "./_reader/SurahReader.svelte";
  import Results from "./_reader/Results.svelte";
  import Player from "./_reader/Player.svelte";
</script>

<Seo path="/app" noindex />

<div
  class="mx-auto grid max-w-[1320px] grid-cols-1 gap-5 px-5 pt-6 sm:px-7 md:grid-cols-[288px_1fr] md:items-start"
>
  <Sidebar />

  <section class="flex min-w-0 flex-col gap-4">
    {#if reader.hasQuery}
      <Results />
    {:else}
      <SurahReader />
    {/if}
  </section>
</div>

<Player />
