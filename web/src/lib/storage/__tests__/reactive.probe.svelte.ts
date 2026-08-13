import { SvelteMap } from "svelte/reactivity";

interface MapSizeObserver {
  reads: () => number;
  dispose: () => void;
}

export function observeMapSize(map: SvelteMap<number, string[]>): MapSizeObserver {
  let count = 0;
  const cleanup = $effect.root(() => {
    $effect(() => {
      void map.size;
      count++;
    });
  });
  return { reads: () => count, dispose: cleanup };
}
