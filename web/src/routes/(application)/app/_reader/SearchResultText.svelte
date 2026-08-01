<script lang="ts">
  import type { Highlight } from "$lib/quran/search/types";
  import { highlightSegments } from "$lib/quran/search/highlights";

  let { text, highlights }: { text: string; highlights: readonly Highlight[] } = $props();
  const segments = $derived(highlightSegments(text, highlights));
</script>

<span dir="rtl" class="font-arabic text-[26px] leading-[2] text-fg">
  {#each segments as segment (`${segment.start}:${segment.end}:${segment.highlighted}`)}
    {#if segment.highlighted}
      <mark class="rounded-sm bg-accent-soft text-inherit">{segment.text}</mark>
    {:else}
      {segment.text}
    {/if}
  {/each}
</span>
