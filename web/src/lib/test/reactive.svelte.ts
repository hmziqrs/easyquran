import { tick } from "svelte";

/**
 * Test-only reactive probe. Lives in a `.svelte.ts` module so the
 * `$effect` / `$effect.root` runes compile. Observes whether a reactive
 * `read()` expression re-runs after external mutation. Requires a DOM
 * environment (happy-dom) for the effect scheduler to run.
 *
 * Usage:
 *   const seen = await track(() => reader.versesFor(num));
 *   expect(seen.runs).toBe(1);
 *   reader.seedSurah(num, verses);
 *   await seen.settle();
 *   expect(seen.runs).toBe(2); // a non-reactive Map would leave this at 1
 */
export async function track<T>(read: () => T): Promise<{
  readonly runs: number;
  readonly value: T;
  settle: () => Promise<void>;
  dispose: () => void;
}> {
  let runs = 0;
  let value!: T;
  const dispose = $effect.root(() => {
    $effect(() => {
      runs++;
      value = read();
    });
  });
  await tick();
  return {
    get runs() {
      return runs;
    },
    get value() {
      return value;
    },
    settle: () => tick(),
    dispose,
  };
}
