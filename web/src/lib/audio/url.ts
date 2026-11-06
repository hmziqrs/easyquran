import type { AudioRecording } from "./catalogue";

export interface AyahRef {
  readonly surah: number;
  readonly ayah: number;
}

export interface ResolvedAyahAudio {
  /** Absolute `https://` URL of the file to play. */
  readonly url: string;
  /** Offset in seconds where this ayah starts inside `url`. */
  readonly startSec: number;
  /** Offset in seconds where this ayah ends, or null to play to the end of the file. */
  readonly endSec: number | null;
}

const SURAH_COUNT = 114;

function pad3(n: number): string {
  return String(n).padStart(3, "0");
}

export function isSurahNumber(n: number): boolean {
  return Number.isInteger(n) && n >= 1 && n <= SURAH_COUNT;
}

/** True for a surah in 1..114 and an ayah >= 1, bounded by `ayahCount` when given. */
export function isValidAyahRef(ref: AyahRef, ayahCount?: number): boolean {
  if (!isSurahNumber(ref.surah)) return false;
  if (!Number.isInteger(ref.ayah) || ref.ayah < 1) return false;
  return ayahCount === undefined || ref.ayah <= ayahCount;
}

/** Fills `{surah}` and `{ayah}` with 3-digit numbers. */
export function expandTemplate(template: string, ref: AyahRef): string {
  return template.replaceAll("{surah}", pad3(ref.surah)).replaceAll("{ayah}", pad3(ref.ayah));
}

/** Playable file and offsets for one ayah, or null when the ref or timing cannot resolve. */
export function resolveAyahAudio(
  recording: AudioRecording,
  ref: AyahRef,
  ayahCount?: number,
): ResolvedAyahAudio | null {
  if (!isValidAyahRef(ref, ayahCount)) return null;
  const timing = recording.timing;
  if (timing.kind === "file-per-ayah") {
    return { url: expandTemplate(recording.urlTemplate, ref), startSec: 0, endSec: null };
  }
  const startMs = timing.ayahStartMs[ref.ayah];
  if (startMs === undefined) return null;
  const nextMs = timing.ayahStartMs[ref.ayah + 1];
  return {
    url: expandTemplate(recording.urlTemplate, ref),
    startSec: startMs / 1000,
    endSec: nextMs === undefined ? null : nextMs / 1000,
  };
}

/** Only `https:` audio is ever loaded, whatever the catalogue says. */
export function isAllowedAudioUrl(url: string): boolean {
  try {
    return new URL(url).protocol === "https:";
  } catch {
    return false;
  }
}
