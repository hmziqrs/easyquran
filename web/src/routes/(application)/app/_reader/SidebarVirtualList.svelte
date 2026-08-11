<script lang="ts">
  import { createVirtualizer } from "@tanstack/svelte-virtual";
  import type { Snippet } from "svelte";

  let {
    getScrollElement,
    count,
    estimateSize = 48,
    overscan = 6,
    activeIndex = -1,
    item,
  }: {
    getScrollElement: () => HTMLElement | null;
    count: number;
    estimateSize?: number;
    overscan?: number;
    /** Row to reveal (centered) when the list mounts or the row changes. */
    activeIndex?: number;
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

  // Rows are measured lazily, so a single scrollToIndex lands short once real
  // heights replace the estimate. Repeat over a few frames to settle.
  let revealed = -1;
  function reveal(index: number, attempt = 0): void {
    requestAnimationFrame(() => {
      if (revealed !== index) return;
      if (!getScrollElement()) {
        if (attempt < 30) reveal(index, attempt + 1);
        return;
      }
      $virtualizer.scrollToIndex(index, { align: "center" });
      if (attempt < 3) reveal(index, attempt + 1);
    });
  }

  $effect(() => {
    const index = activeIndex;
    const total = count;
    if (index < 0 || index >= total || revealed === index) return;
    revealed = index;
    reveal(index);
  });
</script>

<div class="relative w-full" style="height:{$virtualizer.getTotalSize()}px">
  <div
    class="absolute inset-x-0 top-0"
    style="transform:translateY({$virtualizer.getVirtualItems()[0]?.start ?? 0}px)"
  >
    {#each $virtualizer.getVirtualItems() as row, i (row.index)}
      <div
        use:measure
        data-index={row.index}
        aria-setsize={count}
        aria-posinset={row.index + 1}
        class="w-full pb-1"
      >
        {@render item(row.index)}
      </div>
    {/each}
  </div>
</div>
