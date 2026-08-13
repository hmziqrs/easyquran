import { browser } from "$app/environment";

import type { ReaderMode } from "./reader-core.svelte";

export function applyReaderPresentation(mode: ReaderMode, fontSize: number): void {
  if (!browser) return;
  document.documentElement.dataset.readerMode = mode;
  document.documentElement.style.setProperty("--reader-arabic-size", `${fontSize}px`);
}
