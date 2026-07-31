import { SvelteMap } from "svelte/reactivity";

/**
 * Test probe: observe how many times a reactive read of `map.get(num)` fires.
 * Uses $effect.root (a rune) so it must live in a .svelte.ts module.
 */
export function observeMapSize(map: SvelteMap<number, string[]>): {
  reads: () => number;
  dispose: () => void;
} {
  let count = 0;
  const cleanup = $effect.root(() => {
    $effect(() => {
      // read a reactive signal derived from the map
      void map.size;
      count++;
    });
  });
  return { reads: () => count, dispose: cleanup };
}
