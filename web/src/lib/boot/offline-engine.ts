import { bootOfflineEngine } from "$lib/quran/offline";

let started = false;
let teardown: (() => void) | null = null;

export function startOfflineEngine(): () => void {
  if (started) return () => {};
  started = true;
  teardown = bootOfflineEngine();
  return () => {
    teardown?.();
    teardown = null;
  };
}
