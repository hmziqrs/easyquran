<script lang="ts">
  import ReaderVirtualList from "../ReaderVirtualList.svelte";
  import type { ReaderVirtualItem } from "../virtual-reader";

  let { items, preserving = false }: { items: readonly ReaderVirtualItem[]; preserving?: boolean } = $props();
  let list: ReaderVirtualList<ReaderVirtualItem> | undefined = $state();

  export async function reveal(key: string, localPage: number): Promise<void> {
    await list?.reveal(key, localPage);
  }

  export function finishReveal(): void {
    preserving = false;
  }
</script>

<ReaderVirtualList bind:this={list} {items} {preserving} tag="ol" layoutKey="test">
  {#snippet item(entry, index, gap, measure)}
    <li
      {@attach measure}
      data-index={index}
      data-local-page={entry.localPage}
      data-verse-key={entry.verseKey}
      style:height={`${entry.estimate}px`}
      style:margin-block-start={`${gap}px`}
    >
      <button>{entry.key}</button>
    </li>
  {/snippet}
</ReaderVirtualList>
