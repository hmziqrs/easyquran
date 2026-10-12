/**
 * Env-free leaf for the derive-only author grouping over the baked translation
 * catalogue (one author, multiple sources). Like text-marks.ts it must stay
 * free of site-config/$app imports so component test graphs stay clean: it
 * imports nothing at runtime. The catalogue itself never changes (derive-only
 * rule); grouping is presentation over `translator` + the id's provenance.
 */

/** The catalogue fields the author grouping derives from. */
export interface TranslationAuthorEntry {
  readonly id: string;
  readonly languageCode: string;
  readonly translator: string | null;
}

/**
 * Author identity: trim + casefold + whitespace-collapse of the translator
 * string ("Muhammad  Hamidullah" and "muhammad hamidullah" are one author).
 * Null/blank translators return "" (the unnamed bucket), which never merges:
 * two unnamed works of a language are unrelated.
 */
export function normalizedTranslatorKey(translator: string | null): string {
  if (translator === null) return "";
  return translator.trim().toLowerCase().replace(/\s+/g, " ");
}

/**
 * Grouping key: the translator within one language. Unnamed translators fall
 * back to a per-id key so they never collapse into one group.
 */
export function translationAuthorKey(entry: TranslationAuthorEntry): string {
  const translator = normalizedTranslatorKey(entry.translator);
  if (translator !== "") return `${entry.languageCode} ${translator}`;
  return `${entry.languageCode} #id:${entry.id}`;
}

/** One author's source variants, in the order they were given. */
export interface TranslationAuthorGroup<T extends TranslationAuthorEntry> {
  readonly key: string;
  readonly variants: T[];
}

/**
 * Groups catalogue entries by author within their language. Group order is
 * first-seen and variant order preserves the input order, so callers control
 * sorting before (and tie-breaking after) the grouping.
 */
export function groupTranslationAuthors<T extends TranslationAuthorEntry>(
  entries: readonly T[],
): TranslationAuthorGroup<T>[] {
  const groups = new Map<string, T[]>();
  for (const entry of entries) {
    const key = translationAuthorKey(entry);
    const existing = groups.get(key);
    if (existing) existing.push(entry);
    else groups.set(key, [entry]);
  }
  return [...groups].map(([key, variants]) => ({ key, variants }));
}

/**
 * Provenance preference for a group's default variant: original publishers
 * (tanzil, quranenc) rank ahead of QUL mirrors, so an untouched author group
 * shows the original work, not its mirror.
 */
export function translationProvenanceRank(id: string): number {
  if (id.startsWith("qul.")) return 2;
  if (id.startsWith("quranenc.")) return 1;
  return 0;
}
