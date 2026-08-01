import { SvelteMap } from "svelte/reactivity";

export function observeMapSize(map: SvelteMap<number, string[]>): {
  reads: () => number;
  dispose: () => void;
} {
  let count = 0;
  const cleanup = $effect.root(() => {
    $effect(() => {
      void map.size;
      count++;
    });
  });
  return { reads: () => count, dispose: cleanup };
}
