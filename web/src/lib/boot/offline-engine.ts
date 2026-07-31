/* ════════════════════════════════════════════════════════════════════════
   boot/offline-engine.ts — lifecycle wrapper around the offline Quran engine.

   startOfflineEngine() boots the ~2.5 MB offline corpus + sqlite-wasm Worker
   (see lib/quran/offline.ts) exactly once, and returns the engine's teardown.

   GATING: this is expensive (corpus download + worker boot), so the caller must
   start it ONLY when the user enters /app — not on marketing routes (see
   routes/+layout.svelte). It is idempotent, so calling it on every /app
   navigation is safe and cheap: the first call boots, subsequent calls return a
   no-op teardown. The reader still renders instantly from prerendered
   page.data regardless of when (or whether) the engine boots.
   ════════════════════════════════════════════════════════════════════════ */

import { bootOfflineEngine } from "$lib/quran/offline";

let started = false;
let teardown: (() => void) | null = null;

/**
 * Start the offline Quran engine once. Idempotent: the first call boots and
 * captures the teardown; later calls are a no-op and return an empty teardown.
 */
export function startOfflineEngine(): () => void {
  if (started) return () => {};
  started = true;
  teardown = bootOfflineEngine();
  return () => {
    teardown?.();
    teardown = null;
  };
}
