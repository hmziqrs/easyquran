<script lang="ts" generics="T extends ReaderVirtualItem">
  import { onMount, tick, untrack, type Snippet } from "svelte";
  import type { Attachment } from "svelte/attachments";
  import { get } from "svelte/store";
  import {
    createWindowVirtualizer,
    measureElement,
    type Range,
    type VirtualItem,
    type Virtualizer,
  } from "@tanstack/svelte-virtual";
  import { clamp } from "es-toolkit";
  import { bufferedIndexes, type ReaderVirtualItem } from "./virtual-reader";
  import type { ViewportAnchor } from "./viewport-anchor";

  let {
    items,
    tag = "div",
    layoutKey,
    preserving = false,
    onRendered,
    onResize,
    item,
  }: {
    items: readonly T[];
    tag?: "div" | "ol";
    layoutKey: string;
    preserving?: boolean;
    onRendered?: (rendered: readonly T[]) => void;
    onResize?: (node: HTMLElement) => void;
    item: Snippet<[T, number, number, Attachment<HTMLElement>]>;
  } = $props();

  let mounted = $state(false);
  let list: HTMLElement | null = $state(null);
  let scrollMargin = $state(0);
  let focusedKey = $state<string | null>(null);
  let previousLayout = "";
  const sizes = new Map<string, number>();

  function extractRange(range: Range, focus: string | null): number[] {
    const buffer = clamp((model.scrollRect?.height ?? 720) * 0.8, 500, 1200);
    const offset = model.scrollOffset ?? 0;
    const start = model.getVirtualItemForOffset(Math.max(0, offset - buffer))?.index ?? range.startIndex;
    const end = model.getVirtualItemForOffset(
      offset + (model.scrollRect?.height ?? 0) + buffer,
    )?.index ?? range.endIndex;
    const pinned = focus === null ? -1 : items.findIndex((entry) => entry.key === focus);
    return bufferedIndexes(start, end, range.count, pinned);
  }

  const virtualizer = createWindowVirtualizer<HTMLElement>({
    count: 0,
    enabled: false,
    estimateSize: () => 200,
  });
  const model = get(virtualizer);
  model.shouldAdjustScrollPositionOnItemSizeChange = (entry, _delta, instance) =>
    !preserving && entry.end < (instance.scrollOffset ?? 0);

  function measureNode(
    node: HTMLElement,
    entry: ResizeObserverEntry | undefined,
    instance: Virtualizer<Window, HTMLElement>,
  ): number {
    const size = measureElement(node, entry, instance);
    const index = Number(node.dataset.index);
    const key = items[index]?.key;
    if (key) {
      const previous = sizes.get(key);
      sizes.set(key, size);
      if (previous !== undefined && Math.abs(previous - size) > 1 && !preserving) onResize?.(node);
    }
    return size;
  }

  $effect(() => {
    const currentLayout = layoutKey;
    const currentItems = items;
    const focus = focusedKey;
    const enabled = mounted;
    const margin = scrollMargin;
    const element = list;
    untrack(() => {
      model.setOptions({
        count: currentItems.length,
        enabled,
        scrollMargin: margin,
        estimateSize: (index) => currentItems[index]?.estimate ?? 200,
        getItemKey: (index) => currentItems[index]?.key ?? index,
        rangeExtractor: (range) => extractRange(range, focus),
        measureElement: measureNode,
        useAnimationFrameWithResizeObserver: true,
      });
      if (previousLayout !== currentLayout) {
        previousLayout = currentLayout;
        sizes.clear();
        model.measure();
        for (const node of element?.querySelectorAll<HTMLElement>("[data-index]") ?? []) {
          model.measureElement(node);
        }
      }
    });
  });

  const rows = $derived.by((): VirtualItem[] => {
    if (mounted) return $virtualizer.getVirtualItems();
    return items.map((entry, index) => ({ key: entry.key, index, start: 0, end: 0, size: 0, lane: 0 }));
  });
  const paddingTop = $derived(
    mounted ? Math.max(0, (rows[0]?.start ?? scrollMargin) - scrollMargin) : 0,
  );
  const paddingBottom = $derived(
    mounted
      ? Math.max(0, $virtualizer.getTotalSize() - ((rows.at(-1)?.end ?? scrollMargin) - scrollMargin))
      : 0,
  );

  $effect(() => {
    const rendered = rows.flatMap((entry) => {
      const value = items[entry.index];
      return value ? [value] : [];
    });
    untrack(() => onRendered?.(rendered));
  });

  const measure: Attachment<HTMLElement> = (node) => {
    model.measureElement(node);
    return () => model.measureElement(null);
  };

  const captureList: Attachment<HTMLElement> = (node) => {
    list = node;
    const updateMargin = () => {
      const margin = node.getBoundingClientRect().top + window.scrollY;
      if (Math.abs(margin - scrollMargin) > 0.5) scrollMargin = margin;
    };
    let frame: number | null = null;
    const observer = new ResizeObserver(() => {
      if (frame !== null) return;
      frame = requestAnimationFrame(() => {
        frame = null;
        updateMargin();
      });
    });
    observer.observe(node);
    updateMargin();
    return () => {
      observer.disconnect();
      if (frame !== null) cancelAnimationFrame(frame);
      if (list === node) list = null;
    };
  };

  function updateFocus(): void {
    const active = document.activeElement;
    if (!active || !list?.contains(active)) {
      focusedKey = null;
      return;
    }
    const node = active.closest<HTMLElement>("[data-index]");
    focusedKey = items[Number(node?.dataset.index)]?.key ?? null;
  }

  function onFocusOut(): void {
    queueMicrotask(updateFocus);
  }

  export async function reveal(verseKey: string | null, localPage: number): Promise<void> {
    let index = items.findIndex((entry) => entry.verseKey === verseKey);
    if (index < 0) index = items.findIndex((entry) => entry.localPage === localPage);
    if (index < 0 || !mounted) return;
    await tick();
    model.scrollToIndex(index, { align: "center", behavior: "auto" });
    await tick();
  }

  export async function prepareAnchor(anchor: ViewportAnchor): Promise<void> {
    const verseKey = anchor.kind === "verse" ? anchor.verseKey : null;
    const node = list?.querySelector<HTMLElement>(`[data-local-page="${anchor.localPage}"]`);
    if (verseKey && list?.querySelector(`[data-verse-key="${verseKey}"]`)) return;
    if (!verseKey && node) return;
    await reveal(verseKey, anchor.localPage);
  }

  onMount(() => {
    mounted = true;
  });
</script>

<svelte:element
  this={tag}
  {@attach captureList}
  class="virtual-reader-list"
  data-reader-virtual-list
  style:padding-top={`${paddingTop}px`}
  style:padding-bottom={`${paddingBottom}px`}
  onfocusin={updateFocus}
  onfocusout={onFocusOut}
>
  {#each rows as row, position (row.key)}
    {@const value = items[row.index]}
    {@const gap = position === 0 ? 0 : Math.max(0, row.start - (rows[position - 1]?.end ?? row.start))}
    {#if value}
      {@render item(value, row.index, gap, measure)}
    {/if}
  {/each}
</svelte:element>

<style>
  .virtual-reader-list {
    display: flow-root;
    list-style: none;
    margin: 0;
    padding-inline: 0;
    overflow-anchor: none;
  }
</style>
