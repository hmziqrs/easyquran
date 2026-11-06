/**
 * Recitation player engine. One module-level instance, so playback and the queue
 * survive SPA navigation. Loaded only through the dock's dynamic import, and only
 * once a recording is licensed (see `catalogue.ts`).
 *
 * Source-agnostic: every file URL comes from `resolveAyahAudio`, and every limit
 * from `policy.ts`. Nothing here names a reciter or a host.
 */
import { clamp } from "es-toolkit";

import type { JsonRecord, JsonValue } from "#lib/storage/decoders.js";
import {
  asArray,
  asNumber,
  asObject,
  asString,
  readJSON,
  tryWriteJSON,
} from "#lib/storage/index.js";

import { recordingById, reciterById, type AudioRecording } from "./catalogue";
import { cacheAudioFile, cachedAudioObjectUrl, type CacheResult } from "./offline";
import { DEFAULT_PLAYBACK_RATE, nearestRate, retryDelayMs, type PlaybackRate } from "./policy";
import {
  MAX_QUEUE_ITEMS,
  buildQueue,
  currentItem,
  isValidSegment,
  queueAt,
  stepQueue,
  surahBoundary,
  type PlayQueue,
  type QueueItem,
  type QueueSegment,
} from "./queue";
import {
  holdScreenAwake,
  normalizePlan,
  planSteps,
  runRepeat,
  waitMs,
  type RepeatPlanInput,
  type RepeatStep,
} from "./repeat";
import { isAllowedAudioUrl, resolveAyahAudio } from "./url";

export type AudioStatus = "idle" | "loading" | "playing" | "paused" | "error";

/** How one ayah play ended. */
type Outcome = "ended" | "error" | "blocked" | "aborted";

/** Thrown when an ayah still fails after every retry. */
export class AudioLoadError extends Error {
  constructor() {
    super("audio-load-failed");
    this.name = "AudioLoadError";
  }
}

export const AUDIO_SESSION_KEY = "easyquran.audio.session.v1";

interface SavedSession {
  readonly recordingId: string;
  readonly segments: readonly QueueSegment[];
  readonly index: number;
  readonly rate: number;
}

function decodeSegment(obj: JsonRecord | undefined): QueueSegment | undefined {
  if (!obj) return undefined;
  const surah = asNumber(obj.surah, 1, 114);
  const from = asNumber(obj.from, 1, 286);
  const to = asNumber(obj.to, 1, 286);
  if (surah === undefined || from === undefined || to === undefined) return undefined;
  const segment = { surah, from, to };
  return isValidSegment(segment) ? segment : undefined;
}

function parseSession(raw: JsonValue | undefined): SavedSession | null {
  const obj = asObject(raw);
  if (!obj) return null;
  const recordingId = asString(obj.recordingId);
  const index = asNumber(obj.index, 0, MAX_QUEUE_ITEMS);
  const rate = asNumber(obj.rate, 0, 2);
  if (recordingId === undefined || index === undefined || rate === undefined) return null;
  const segments = asArray(obj.segments, (item) => decodeSegment(asObject(item)));
  return { recordingId, segments, index, rate };
}

function isNotAllowed(error: Error): boolean {
  return error.name === "NotAllowedError";
}

class AudioEngine {
  status = $state<AudioStatus>("idle");
  recordingId = $state<string | null>(null);
  queue = $state.raw<PlayQueue | null>(null);
  /** The repeat step now playing, or null outside a repeat run. */
  repeatStep = $state.raw<RepeatStep | null>(null);
  /** Seconds into the current ayah. */
  positionSec = $state(0);
  /** Length of the current ayah in seconds. */
  durationSec = $state(0);
  rate = $state<PlaybackRate>(DEFAULT_PLAYBACK_RATE);
  /** Retry number while retrying a failed load, else 0. */
  attempt = $state(0);
  /** True while a queue is being saved for offline listening. */
  caching = $state(false);
  /** Whether the full player panel is open. Owned here so reader actions can open it. */
  panelOpen = $state(false);
  /** A range the reader asked the repeat panel to load. `seq` changes on every request. */
  repeatRequest = $state.raw<{
    readonly surah: number;
    readonly from: number;
    readonly to: number;
    readonly seq: number;
  } | null>(null);

  #el: HTMLAudioElement | null = null;
  #settle: ((outcome: Outcome) => void) | null = null;
  #token: object | null = null;
  #startSec = 0;
  #endSec: number | null = null;
  #run: AbortController | null = null;
  #objectUrl: string | null = null;
  #releaseWake: (() => void) | null = null;
  #hydrated = false;

  get recording(): AudioRecording | undefined {
    return this.recordingId === null ? undefined : recordingById(this.recordingId);
  }

  get item(): QueueItem | undefined {
    return this.queue === null ? undefined : currentItem(this.queue);
  }

  /** Restores the saved queue as paused. Never autoplays. Runs once per page load. */
  hydrate(): void {
    if (this.#hydrated) return;
    this.#hydrated = true;
    const saved = parseSession(readJSON(AUDIO_SESSION_KEY));
    if (!saved || !recordingById(saved.recordingId)) return;
    const queue = buildQueue(saved.segments);
    if (!queue) return;
    this.recordingId = saved.recordingId;
    this.queue = queueAt(queue, saved.index) ?? queue;
    this.rate = nearestRate(saved.rate);
    this.status = "paused";
  }

  /** Replaces the queue and starts it from its first ayah. */
  playQueue(recordingId: string, segments: readonly QueueSegment[]): boolean {
    if (!recordingById(recordingId)) return false;
    const queue = buildQueue(segments);
    if (!queue) return false;
    this.recordingId = recordingId;
    this.queue = queue;
    this.#persist();
    this.#runQueue();
    return true;
  }

  play(): void {
    const el = this.#el;
    if (el && this.#run && this.#settle && el.paused) {
      void el.play().catch(() => {});
      return;
    }
    if (this.queue) this.#runQueue();
  }

  pause(): void {
    this.#el?.pause();
    if (this.status === "playing" || this.status === "loading") this.status = "paused";
  }

  toggle(): void {
    if (this.status === "playing" || this.status === "loading") this.pause();
    else this.play();
  }

  next(): void {
    if (this.queue) this.#moveTo(stepQueue(this.queue, 1));
  }

  prev(): void {
    if (this.queue) this.#moveTo(stepQueue(this.queue, -1));
  }

  nextSurah(): void {
    const index = this.#boundary(1);
    if (index !== null) this.jumpTo(index);
  }

  prevSurah(): void {
    const index = this.#boundary(-1);
    if (index !== null) this.jumpTo(index);
  }

  jumpTo(index: number): void {
    if (!this.queue) return;
    this.#moveTo(queueAt(this.queue, index));
  }

  /** Seconds into the current ayah. Clamped to its length. */
  seek(sec: number): void {
    const el = this.#el;
    if (!el || !Number.isFinite(sec)) return;
    const span = this.durationSec;
    el.currentTime = this.#startSec + clamp(sec, 0, span > 0 ? span : 0);
  }

  setRate(value: number): void {
    const rate = nearestRate(value);
    this.rate = rate;
    if (this.#el) this.#el.playbackRate = rate;
    this.#persist();
  }

  setPanelOpen(open: boolean): void {
    this.panelOpen = open;
  }

  /** Opens the full player with the repeat panel set to this range. */
  requestRepeat(surah: number, from: number, to: number): void {
    const seq = (this.repeatRequest?.seq ?? 0) + 1;
    this.repeatRequest = { surah, from, to, seq };
    this.panelOpen = true;
  }

  /** Retries the current ayah after a failure. */
  retry(): void {
    if (this.queue) this.#runQueue();
  }

  /** Stops playback and any repeat run. Keeps the queue so the dock stays put. */
  stop(): void {
    this.#cancel();
    this.repeatStep = null;
    this.status = this.queue ? "paused" : "idle";
  }

  /** Starts a repeat run over the given range. False when the plan is invalid. */
  startRepeat(recordingId: string, input: RepeatPlanInput, ayahCount: number): boolean {
    const recording = recordingById(recordingId);
    const plan = recording ? normalizePlan(input, ayahCount) : null;
    if (!recording || !plan) return false;
    this.#cancel();
    this.recordingId = recording.id;
    const controller = new AbortController();
    const signal = controller.signal;
    this.#run = controller;
    this.#releaseWake = holdScreenAwake();
    const steps = planSteps(plan);
    void (async () => {
      try {
        const result = await runRepeat(
          steps,
          plan.delayMs,
          {
            play: async (step, playSignal) => {
              const outcome = await this.#playItem(recording, step, playSignal);
              if (outcome === "blocked") throw new AudioLoadError();
            },
            wait: waitMs,
            onStep: (step) => {
              this.repeatStep = step;
            },
          },
          signal,
        );
        if (result === "done") this.status = "idle";
      } catch {
        if (!signal.aborted) this.status = "error";
      } finally {
        if (this.#run === controller) {
          this.#run = null;
          this.repeatStep = null;
          this.#releaseWakeLock();
        }
      }
    })();
    return true;
  }

  stopRepeat(): void {
    if (this.repeatStep === null) return;
    this.stop();
  }

  /** Saves every file of the current queue for offline listening. */
  async cacheQueue(): Promise<CacheResult> {
    const recording = this.recording;
    const queue = this.queue;
    if (!recording || !queue) return "failed";
    const urls = new Set<string>();
    for (const item of queue.items) {
      const resolved = resolveAyahAudio(recording, item);
      if (resolved) urls.add(resolved.url);
    }
    this.caching = true;
    try {
      for (const url of urls) {
        if (!isAllowedAudioUrl(url)) return "failed";
        const result = await cacheAudioFile(url);
        if (result !== "ok") return result;
      }
      return "ok";
    } finally {
      this.caching = false;
    }
  }

  #boundary(direction: 1 | -1): number | null {
    return this.queue === null ? null : surahBoundary(this.queue, direction);
  }

  #moveTo(next: PlayQueue | null): void {
    if (next === null) return;
    this.queue = next;
    this.#persist();
    this.#runQueue();
  }

  #element(): HTMLAudioElement {
    if (this.#el) return this.#el;
    const el = new Audio();
    el.preload = "auto";
    el.addEventListener("loadedmetadata", () => {
      if (this.#startSec > 0) el.currentTime = this.#startSec;
      const end = this.#endSec ?? el.duration;
      this.durationSec = Number.isFinite(end) ? Math.max(0, end - this.#startSec) : 0;
    });
    el.addEventListener("timeupdate", () => {
      this.positionSec = Math.max(0, el.currentTime - this.#startSec);
      if (this.#endSec !== null && el.currentTime >= this.#endSec) {
        el.pause();
        this.#settleWith("ended");
      }
    });
    el.addEventListener("ended", () => this.#settleWith("ended"));
    el.addEventListener("error", () => this.#settleWith("error"));
    el.addEventListener("playing", () => {
      this.status = "playing";
      this.attempt = 0;
    });
    el.addEventListener("waiting", () => {
      if (this.status === "playing") this.status = "loading";
    });
    el.addEventListener("pause", () => {
      if (this.status === "playing") this.status = "paused";
    });
    this.#el = el;
    this.#wireMediaSession();
    return el;
  }

  #wireMediaSession(): void {
    if (!("mediaSession" in navigator)) return;
    const session = navigator.mediaSession;
    const handlers: readonly [MediaSessionAction, () => void][] = [
      ["play", () => this.play()],
      ["pause", () => this.pause()],
      ["previoustrack", () => this.prev()],
      ["nexttrack", () => this.next()],
      ["seekbackward", () => this.seek(this.positionSec - 10)],
      ["seekforward", () => this.seek(this.positionSec + 10)],
    ];
    for (const [action, handler] of handlers) {
      try {
        session.setActionHandler(action, handler);
      } catch {
        // Browser does not support this action; the others still work.
      }
    }
  }

  #updateMediaSession(): void {
    if (!("mediaSession" in navigator)) return;
    const item = this.item;
    const recording = this.recording;
    if (!item || !recording) return;
    try {
      navigator.mediaSession.metadata = new MediaMetadata({
        title: `${item.surah}:${item.ayah}`,
        artist: reciterById(recording.reciterId)?.name ?? "",
        album: recording.label,
      });
    } catch {
      // Metadata is decoration; playback does not depend on it.
    }
  }

  #settleWith(outcome: Outcome): void {
    const settle = this.#settle;
    this.#settle = null;
    settle?.(outcome);
  }

  #releaseObjectUrl(): void {
    if (this.#objectUrl === null) return;
    URL.revokeObjectURL(this.#objectUrl);
    this.#objectUrl = null;
  }

  #releaseWakeLock(): void {
    this.#releaseWake?.();
    this.#releaseWake = null;
  }

  /** Stops the active run (queue or repeat) and frees its resources. */
  #cancel(): void {
    this.#run?.abort();
    this.#run = null;
    this.#settleWith("aborted");
    this.#el?.pause();
    this.#releaseWakeLock();
  }

  #runQueue(): void {
    this.#cancel();
    const controller = new AbortController();
    this.#run = controller;
    void this.#drive(controller);
  }

  async #drive(controller: AbortController): Promise<void> {
    const signal = controller.signal;
    try {
      while (!signal.aborted) {
        const queue = this.queue;
        const recording = this.recording;
        const item = queue ? currentItem(queue) : undefined;
        if (!queue || !recording || !item) {
          this.status = "error";
          return;
        }
        this.#updateMediaSession();
        const outcome = await this.#playItem(recording, item, signal);
        if (outcome !== "ended") return;
        const next = stepQueue(queue, 1);
        if (!next) {
          this.status = "idle";
          return;
        }
        this.queue = next;
        this.#persist();
      }
    } catch {
      if (!signal.aborted) this.status = "error";
    } finally {
      if (this.#run === controller) this.#run = null;
    }
  }

  /**
   * Plays one ayah with retries. Resolves "ended" when it finishes, "aborted" when
   * the signal cancels it, "blocked" when the browser refuses autoplay. Throws
   * AudioLoadError once every retry has failed.
   */
  async #playItem(
    recording: AudioRecording,
    item: { readonly surah: number; readonly ayah: number },
    signal: AbortSignal,
  ): Promise<Outcome> {
    for (let attempt = 0; ; attempt++) {
      if (signal.aborted) return "aborted";
      const resolved = resolveAyahAudio(recording, item);
      if (!resolved || !isAllowedAudioUrl(resolved.url)) throw new AudioLoadError();
      this.status = "loading";
      const local = await cachedAudioObjectUrl(resolved.url);
      if (signal.aborted) {
        if (local) URL.revokeObjectURL(local);
        return "aborted";
      }
      const el = this.#element();
      this.#releaseObjectUrl();
      this.#objectUrl = local;
      this.#startSec = resolved.startSec;
      this.#endSec = resolved.endSec;
      this.positionSec = 0;
      this.durationSec = 0;
      el.playbackRate = this.rate;

      const token = {};
      this.#token = token;
      const outcome = new Promise<Outcome>((resolve) => {
        this.#settle = resolve;
      });
      const onAbort = (): void => {
        el.pause();
        this.#settleWith("aborted");
      };
      signal.addEventListener("abort", onAbort, { once: true });
      el.src = local ?? resolved.url;
      el.play().catch((error: Error) => {
        if (this.#token !== token) return;
        this.#settleWith(isNotAllowed(error) ? "blocked" : "error");
      });

      const result = await outcome;
      signal.removeEventListener("abort", onAbort);
      if (result === "ended" || result === "aborted") return result;
      if (result === "blocked") {
        this.status = "paused";
        return "blocked";
      }
      const wait = retryDelayMs(attempt + 1);
      if (wait === null) throw new AudioLoadError();
      this.attempt = attempt + 1;
      this.status = "loading";
      await waitMs(wait, signal);
    }
  }

  #persist(): void {
    const queue = this.queue;
    if (!queue || this.recordingId === null) return;
    tryWriteJSON(AUDIO_SESSION_KEY, {
      recordingId: this.recordingId,
      segments: queue.segments,
      index: queue.index,
      rate: this.rate,
    });
  }
}

export const audio = new AudioEngine();
