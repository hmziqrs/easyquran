import type { ReaderApi } from "../reader.svelte";

/**
 * Test probe: observe a reader's `versesFor(num)` through a real `$effect` so a
 * reactive SvelteMap re-runs the effect on `seedSurah`. Must live in a
 * `.svelte.ts` module because `$effect.root`/`$effect` are runes.
 */
export function observeVersesFor(
  reader: ReaderApi,
  num: number,
): {
  runs: () => number;
  latest: () => string[];
  dispose: () => void;
} {
  let runs = 0;
  let latest: string[] = [];
  const dispose = $effect.root(() => {
    $effect(() => {
      latest = reader.versesFor(num);
      runs++;
    });
  });
  return { runs: () => runs, latest: () => latest, dispose };
}
