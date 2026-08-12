import type { Component } from "svelte";

/**
 * The palette pulls in the Command primitives, the Quran catalogue and every
 * registered source — none of which any page needs until someone actually
 * searches. Keeping it behind a memoized `import()` is what holds it out of the
 * initial bundle; importing `GlobalSearchPalette.svelte` anywhere statically
 * would silently undo that.
 *
 * Guarded by `web/src/lib/components/search/__tests__/lazy-palette.test.ts`.
 */
let pending: Promise<Component> | null = null;

export function loadPalette(): Promise<Component> {
  pending ??= import("./GlobalSearchPalette.svelte").then((m) => m.default as Component);
  return pending;
}
