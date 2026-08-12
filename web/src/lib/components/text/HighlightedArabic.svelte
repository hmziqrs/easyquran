<script lang="ts">
  import type { Highlight } from "$lib/quran/search/types";
  import { highlightSegments } from "$lib/quran/search/highlights";
  import { cn } from "$lib/utils";

  let {
    text,
    highlights,
    class: className,
  }: { text: string; highlights: readonly Highlight[]; class?: string } = $props();
  const segments = $derived(highlightSegments(text, highlights));
</script>

<span lang="ar" dir="rtl" class={cn("font-arabic text-[26px] leading-[2] text-fg", className)}>
  {#each segments as segment (`${segment.start}:${segment.end}:${segment.highlighted}`)}
    {#if segment.highlighted}
      <mark class="rounded-sm bg-accent-soft text-inherit">{segment.text}</mark>
    {:else}
      {segment.text}
    {/if}
  {/each}
</span>
