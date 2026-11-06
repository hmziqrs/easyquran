/**
 * Recitation source catalogue. EMPTY ON PURPOSE: no recitation source is
 * licensed yet (O22). While `AUDIO_CATALOGUE.recordings` is empty:
 *   - no player UI renders (`AUDIO_ENABLED` is false, the layout never imports the dock),
 *   - no audio module loads (the dock is a dynamic import; nothing else pulls it in).
 *
 * How to add a source once licensed:
 *   1. Add the reciter and the recording below. `urlTemplate` must be `https://`.
 *      Placeholders: `{surah}` -> 3-digit surah, `{ayah}` -> 3-digit ayah (`001`).
 *      Use `timing: { kind: "file-per-ayah" }` when each ayah is its own file, or
 *      `timing: { kind: "surah-file", ayahStartMs }` when one file per surah carries
 *      verse offsets in milliseconds (keys are ayah numbers, values are start ms).
 *   2. Fill `licence` and `attribution` from the source's own terms. The attribution
 *      text is shown in the full player, so only state what the licence requires.
 *   3. CSP: the site CSP has no `media-src` today, so it falls back to
 *      `default-src 'self'` and audio from any other origin is blocked. Add these
 *      exact entries (replace `<audio-host>` with the origin only, e.g.
 *      `https://audio.example.org`, never a wildcard):
 *        media-src 'self' blob: <audio-host>
 *        connect-src ... <audio-host>      (offline caching fetches; the host must send CORS headers)
 *      Apply them in every place the CSP is built or written, all four:
 *        - web/src/hooks.server.ts        (`connectSrc` array, plus a new `media-src` entry in the policy list)
 *        - web/server.ts                  (around line 249, same two directives)
 *        - web/static/_headers            (the Content-Security-Policy line)
 *        - web/src/lib/server/preview-headers.ts (preview CSP, mirror of hooks.server.ts)
 *      `blob:` is required because offline playback creates object URLs.
 *   4. Add a test for the new source next to `__tests__/url.test.ts`.
 *
 * Nothing else in `web/src/lib/audio/` names a reciter or host.
 */

export interface AudioLicence {
  /** Short name shown to readers, e.g. the licence identifier. */
  readonly name: string;
  /** `https://` link to the licence text. */
  readonly url: `https://${string}`;
}

export interface AudioAttribution {
  /** Credit line in English. Shown verbatim in the full player. */
  readonly en: string;
  /** Credit line in Arabic. Shown verbatim in the full player. */
  readonly ar: string;
  /** `https://` link to the source the audio came from. */
  readonly url: `https://${string}`;
}

export interface AudioReciter {
  readonly id: string;
  readonly name: string;
}

export type VerseTiming =
  | { readonly kind: "file-per-ayah" }
  | {
      readonly kind: "surah-file";
      /** Ayah number -> start offset in milliseconds inside the surah file. */
      readonly ayahStartMs: Readonly<Record<number, number>>;
    };

export interface AudioRecording {
  readonly id: string;
  readonly reciterId: string;
  /** Display label, e.g. "Murattal, 128 kbps". */
  readonly label: string;
  /** Template with `{surah}` and `{ayah}` placeholders. See the file header. */
  readonly urlTemplate: `https://${string}`;
  readonly timing: VerseTiming;
  readonly licence: AudioLicence;
  readonly attribution: AudioAttribution;
}

export interface AudioCatalogue {
  readonly reciters: readonly AudioReciter[];
  readonly recordings: readonly AudioRecording[];
}

export const AUDIO_CATALOGUE: AudioCatalogue = {
  reciters: [],
  recordings: [],
};

/** Gate for every audio surface. False until a recording is licensed and listed above. */
export const AUDIO_ENABLED: boolean = AUDIO_CATALOGUE.recordings.length > 0;

/** The recording reader actions play with: the first one listed. Undefined while empty. */
export function defaultRecordingId(): string | undefined {
  return AUDIO_CATALOGUE.recordings[0]?.id;
}

export function recordingById(id: string): AudioRecording | undefined {
  return AUDIO_CATALOGUE.recordings.find((recording) => recording.id === id);
}

export function reciterById(id: string): AudioReciter | undefined {
  return AUDIO_CATALOGUE.reciters.find((reciter) => reciter.id === id);
}
