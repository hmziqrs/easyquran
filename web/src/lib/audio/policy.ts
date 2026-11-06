/**
 * Numeric policy for the audio engine: playback speed, retries, offline storage
 * and repeat limits. Kept pure so the limits are tested in one place.
 */
import { clamp } from "es-toolkit";

export const PLAYBACK_RATES = [0.75, 1, 1.25, 1.5, 2] as const;
export type PlaybackRate = (typeof PLAYBACK_RATES)[number];
export const DEFAULT_PLAYBACK_RATE: PlaybackRate = 1;

/** Snaps any number to the nearest allowed playback rate. */
export function nearestRate(value: number): PlaybackRate {
  if (!Number.isFinite(value)) return DEFAULT_PLAYBACK_RATE;
  let best: PlaybackRate = DEFAULT_PLAYBACK_RATE;
  let bestGap = Infinity;
  for (const rate of PLAYBACK_RATES) {
    const gap = Math.abs(rate - value);
    if (gap < bestGap) {
      best = rate;
      bestGap = gap;
    }
  }
  return best;
}

/** Load attempts after the first failure before the player gives up. */
export const MAX_LOAD_RETRIES = 3;

/** Backoff before retry `attempt` (1-based): 1 s, 2 s, 4 s. Null once retries run out. */
export function retryDelayMs(attempt: number): number | null {
  if (!Number.isInteger(attempt) || attempt < 1 || attempt > MAX_LOAD_RETRIES) return null;
  return 1000 * 2 ** (attempt - 1);
}

/** Offline audio budget for this device: 300 MB total, 20 MB per file. */
export const OFFLINE_AUDIO_MAX_BYTES = 300 * 1024 * 1024;
export const OFFLINE_AUDIO_MAX_FILE_BYTES = 20 * 1024 * 1024;

/** True when a file of `fileBytes` fits the budget after `usedBytes` already stored. */
export function admitOfflineBytes(usedBytes: number, fileBytes: number): boolean {
  if (!Number.isFinite(fileBytes) || fileBytes <= 0) return false;
  if (fileBytes > OFFLINE_AUDIO_MAX_FILE_BYTES) return false;
  return usedBytes + fileBytes <= OFFLINE_AUDIO_MAX_BYTES;
}

export const REPEAT_LIMITS = {
  ayahRepeat: { min: 1, max: 20 },
  setRepeat: { min: 1, max: 100 },
  delayMs: { min: 0, max: 10_000 },
} as const;

/** Total plays one repeat run may schedule. */
export const MAX_REPEAT_STEPS = 5000;

export function clampInt(
  value: number,
  limit: { readonly min: number; readonly max: number },
): number {
  if (!Number.isFinite(value)) return limit.min;
  return clamp(Math.round(value), limit.min, limit.max);
}
