import { translationSegmentsFromId } from "$lib/data/quran";
import { TRANSLATION_CATALOGUE_BY_ID } from "$lib/quran/catalogue";
import type { TranslationCatalogueEntry } from "$lib/data/quran-types";
import { hrefFor, positionOf } from "./translation-nav";

/**
 * Reading-mode switch guard (user findings U7/U8): switching to reading mode
 * from any reader UI control must be confirmed first while a translation is
 * in play, because reading mode renders exactly one translation at a time.
 * Arabic primary with zero stacked extras needs no confirmation.
 */
export interface ReadingCandidate {
  /** null = Arabic (the current route source; never a stacked extra). */
  readonly id: string | null;
  /** Catalogue entry for translation candidates; null for Arabic. */
  readonly entry: TranslationCatalogueEntry | null;
}

/**
 * Candidates the confirmation dialog offers: the current source first
 * (route translation primary, or Arabic when the primary is Arabic), then
 * each stacked extra (route primary excluded). Empty array means no
 * confirmation is needed (Arabic primary, no extras).
 */
export function readingCandidates(
  primaryId: string | null,
  stackedIds: readonly string[],
): ReadingCandidate[] {
  const extras = stackedIds.filter(
    (id) => id !== primaryId && TRANSLATION_CATALOGUE_BY_ID.has(id),
  );
  if (primaryId === null && extras.length === 0) return [];
  const out: ReadingCandidate[] = [];
  if (primaryId === null) {
    out.push({ id: null, entry: null });
  } else {
    const entry = TRANSLATION_CATALOGUE_BY_ID.get(primaryId);
    if (entry) out.push({ id: primaryId, entry });
    else out.push({ id: null, entry: null });
  }
  for (const id of extras) {
    const entry = TRANSLATION_CATALOGUE_BY_ID.get(id);
    if (entry) out.push({ id, entry });
  }
  return out;
}

/**
 * Canonical href (with ?mode=reading) for confirming a candidate that differs
 * from the current route source, preserving the reading position via the
 * translation-nav helpers. null when no navigation is needed (Arabic choice
 * or the candidate already is the route primary).
 */
export function readingModeHrefFor(
  candidate: ReadingCandidate,
  primaryId: string | null,
  pathname: string,
): `/app/${string}` | null {
  if (candidate.id === null || candidate.id === primaryId || !candidate.entry) {
    return null;
  }
  const seg = translationSegmentsFromId(candidate.entry.id);
  const href = hrefFor(positionOf(pathname), {
    id: candidate.entry.id,
    lang: seg.lang,
    translator: seg.translator,
  });
  if (!href) return null;
  return `${href}?mode=reading`;
}

/**
 * Tracks whether the CURRENT reading-mode session was entered through a
 * reader UI control (confirmation dialog / direct Arabic switch). The
 * ReaderShell auto-banner is the fallback for URL-initiated transitions
 * only (?mode=reading links with no gesture to confirm against), so it is
 * suppressed while this flag is set. Reset whenever the reader leaves
 * reading mode.
 */
class ReadingModeUiState {
  appliedByUi = $state(false);

  mark(): void {
    this.appliedByUi = true;
  }

  reset(): void {
    this.appliedByUi = false;
  }
}

export const readingModeUi = new ReadingModeUiState();

/** ReaderShell banner visibility: URL-initiated reading transitions only. */
export function readingBannerVisible(opts: {
  reading: boolean;
  hiddenCount: number;
  dismissed: boolean;
}): boolean {
  return opts.reading && opts.hiddenCount > 0 && !opts.dismissed && !readingModeUi.appliedByUi;
}

/**
 * Fresh-reader-mount reset for the UI-gesture flag: a reader mounting OUTSIDE
 * reading mode clears any stale mark left by a reader that unmounted in
 * reading mode (settings route, direct navigation) so a later URL-initiated
 * ?mode=reading keeps its fallback banner. Mounting INTO reading mode keeps
 * the flag: that is the UI-confirmed cross-translation goto path, where the
 * old reader unmounts only AFTER the new shell mounts (an unmount-time reset
 * would re-arm the banner against the transition the user just confirmed).
 */
export function resetUnlessReading(isReadingMode: boolean): void {
  if (!isReadingMode) readingModeUi.reset();
}
