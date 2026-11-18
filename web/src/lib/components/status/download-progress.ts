import { sumBy } from "es-toolkit";

import type { DownloadProgress } from "#lib/data/quran-types.js";
import { isArabicSourceId } from "#lib/data/quran-types.js";

export interface ScriptArtifact {
  readonly id: string;
  readonly sizeBytes: number;
}

/**
 * Fraction (0..1) of the whole Quran download run. Arabic scripts download in
 * `scripts` order, so finished scripts count in full and the active one counts
 * its loaded bytes. A translation download is measured on its own.
 */
export function quranRunFraction(
  download: DownloadProgress | null,
  scripts: readonly ScriptArtifact[],
): number | null {
  if (!download) return null;
  if (!isArabicSourceId(download.script)) {
    return download.total > 0 ? Math.min(1, download.loaded / download.total) : null;
  }
  const totalBytes = sumBy(scripts, (s) => s.sizeBytes);
  if (totalBytes <= 0) return null;
  let done = 0;
  for (const s of scripts) {
    if (s.id === download.script) {
      return Math.min(1, (done + Math.min(download.loaded, s.sizeBytes)) / totalBytes);
    }
    done += s.sizeBytes;
  }
  return null;
}

/** Whole-percent value for a fraction; null reads as 0. */
export function toPercent(fraction: number | null): number {
  return fraction === null ? 0 : Math.round(fraction * 100);
}

/** 25%-step label for screen readers, so a live region does not announce every byte. */
export function milestonePercent(pct: number): number {
  return Math.min(100, Math.floor(pct / 25) * 25);
}
