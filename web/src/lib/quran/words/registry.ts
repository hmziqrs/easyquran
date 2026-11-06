import type { WordSource } from "./types.ts";

/**
 * Word sources. Empty until the owner licenses a source (O23). Add an entry
 * here only with its licence, attribution and a bundled table. While this is
 * empty, VerseRow loads no word code and renders exactly as before.
 */
export const WORD_SOURCES: readonly WordSource[] = Object.freeze([]);

/** Throws on any source that would ship without licence, credit or a valid id. */
export function validateWordSources(sources: readonly WordSource[]): void {
  const seen = new Set<string>();
  for (const source of sources) {
    if (source.id.trim() === "") throw new Error("[words] source with empty id");
    if (seen.has(source.id)) throw new Error(`[words] duplicate source id: ${source.id}`);
    seen.add(source.id);
    if (source.licence.trim() === "") throw new Error(`[words] ${source.id}: no licence`);
    if (source.attribution.trim() === "") throw new Error(`[words] ${source.id}: no attribution`);
    if (source.meaningLanguage.trim() === "") throw new Error(`[words] ${source.id}: no language`);
  }
}
