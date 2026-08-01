import type { ReaderApi } from "../reader.svelte";

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
