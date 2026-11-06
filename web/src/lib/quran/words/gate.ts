import { QuranScript } from "#lib/data/quran-types.js";

import type { WordSource } from "./types.ts";

/**
 * The source that drives word popovers for a reader script, or undefined.
 * Word tokens cover the plain Arabic scripts only: Tajweed carries inline
 * markup and IndoPak has its own glyph renderer, so both keep their views.
 */
export function wordSourceFor(
  script: QuranScript,
  sources: readonly WordSource[],
): WordSource | undefined {
  if (script !== QuranScript.Uthmani && script !== QuranScript.SimpleClean) return undefined;
  return sources[0];
}
