import type { ReadingText } from "#lib/stores/reading-text.svelte.js";

/** One-tap picks the Reading picker shows above its search. */
export const READING_QUICK_MAX = 8;

/**
 * Reading's translation picker, passed to TranslationModal to turn it from the stacked
 * multi-select into a single pick of the text Reading flows.
 */
export interface ReadPick {
  /** The translation flowing now, or null while Reading shows the Arabic. */
  readonly current: string | null;
  /** Quick picks, most relevant first (see readingQuickPicks). */
  readonly quick: readonly string[];
  readonly onPick: (id: string) => void;
}

/**
 * The picker's one-tap chips: the route's own translation (a /t/ URL promises that text),
 * then the reader's recent Reading picks, then their stacked translations — deduped.
 */
export function readingQuickPicks(
  routeTranslationId: string | null,
  recent: readonly string[],
  stackedIds: readonly string[],
): string[] {
  const out: string[] = [];
  for (const id of [routeTranslationId, ...recent, ...stackedIds]) {
    if (id !== null && !out.includes(id)) out.push(id);
  }
  return out.slice(0, READING_QUICK_MAX);
}

/**
 * The translation Reading flows, or null for the Arabic.
 *
 * A translation route always flows a translation — the one picked on this page, else the
 * route's own (its Arabic lives at the Arabic URL). An Arabic route follows the saved
 * choice: the Arabic, or the saved translation (any catalogue translation, stacked or not),
 * else the first stacked one. With nothing to read it falls back to the Arabic.
 */
export function readingFlowId(
  text: ReadingText,
  savedId: string | null,
  routeTranslationId: string | null,
  pagePick: string | null,
  stackedIds: readonly string[],
): string | null {
  if (routeTranslationId !== null) return pagePick ?? routeTranslationId;
  if (text === "arabic") return null;
  return savedId ?? stackedIds[0] ?? null;
}
