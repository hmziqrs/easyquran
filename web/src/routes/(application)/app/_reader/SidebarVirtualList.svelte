<script lang="ts">
  import { createVirtualizer } from "@tanstack/svelte-virtual";
  import type { Snippet } from "svelte";

  let {
    getScrollElement,
    count,
    estimateSize = 48,
    overscan = 6,
    item,
  }: {
    getScrollElement: () => HTMLElement | null;
    count: number;
    estimateSize?: number;
    overscan?: number;
    item: Snippet<[index: number]>;
  } = $props();

  const virtualizer = createVirtualizer<HTMLElement, HTMLElement>({
    get count() {
      return count;
    },
    get getScrollElement() {
      return getScrollElement;
    },
    estimateSize: () => estimateSize,
    get overscan() {
      return overscan;
    },
  });

  const measure = (el: HTMLElement) => $virtualizer.measureElement(el);
</script>

<div class="relative w-full" style="height:{$virtualizer.getTotalSize()}px">
  <div
    class="absolute inset-x-0 top-0"
    style="transform:translateY({$virtualizer.getVirtualItems()[0]?.start ?? 0}px)"
  >
    {#each $virtualizer.getVirtualItems() as row, i (row.index)}
      <div use:measure data-index={row.index} class="w-full pb-1">
        {@render item(row.index)}
      </div>
    {/each}
  </div>
</div>
