import type { QuranReaderSource } from "$lib/data/quran-types";
import { noteReaderView } from "./engagement";

export function trackReaderView(deps: {
  key: () => string;
  sourceId: () => QuranReaderSource | null | undefined;
}): void {
  $effect(() => {
    void deps.key();
    void noteReaderView(deps.sourceId());
  });
}
