import { readingAnalytics } from "#lib/analytics/reading-analytics.js";
import { parseVerseKey } from "#lib/sessions/schema.js";
import { sessions } from "#lib/sessions/store.svelte.js";
import { reader } from "#lib/stores/reader.svelte.js";

/**
 * What counts as "read" (docs/reading-model.md §4.1) — observed from the DOM,
 * so every reader mode (verse rows, reading pages, ranges, snippets) is
 * covered by its `[data-verse-anchor]` markers with no per-reader wiring.
 *
 * A verse is read when it leaves the viewport upward — or the reader moves
 * on / hides while it is still visible — having been on screen at least
 * `max(MIN_DWELL_MS, letters × MS_PER_LETTER)`. 10 ms per letter is roughly
 * 25 words a second: well past the fastest hadr recitation, so a hafiz reading
 * fast always counts and only flings and jumps fall below it. Leaving at the
 * bottom (scrolling back) does not count; the same verse counts at most once
 * per REPEAT_GAP_MS (edge jitter), so real repetition still counts.
 *
 * Time: active wall time (tab visible, an interaction within IDLE_MS — long
 * enough for a slow page recitation without touching the screen) is split
 * across the verses read in that window by letter count.
 */

export const MIN_DWELL_MS = 150;
export const MS_PER_LETTER = 10;
export const IDLE_MS = 5 * 60 * 1000;
const REPEAT_GAP_MS = 30_000;
/** Average Arabic letters per ayah — used when the verse text is not cached yet. */
const FALLBACK_LETTERS = 56;
const WINDOW_FLUSH_MS = 10_000;
const ACTIVITY_EVENTS = ["scroll", "pointerdown", "touchstart", "wheel"] as const;
const ARABIC_LETTER = /[ء-يٱ-ۓ]/gu;

export function dwellFloorMs(letters: number): number {
  return Math.max(MIN_DWELL_MS, letters * MS_PER_LETTER);
}

export function arabicLetterCount(text: string): number {
  return text.match(ARABIC_LETTER)?.length ?? 0;
}

interface Watch {
  readonly key: string;
  readonly surah: number;
  readonly ayah: number;
  /** Intersecting the viewport (independent of tab visibility). */
  inView: boolean;
  visibleSince: number | null;
  visibleMs: number;
}

interface Pending {
  readonly surah: number;
  readonly ayah: number;
  readonly letters: number;
  readonly sessionId: string | null;
  readonly sourceId: string | null;
}

/** Injectable clock + sinks keep the counting rules unit-testable. */
export interface ReadTrackerDeps {
  readonly now?: () => number;
  readonly lettersFor?: (surah: number, ayah: number) => number;
  readonly onRead?: (
    surah: number,
    ayah: number,
  ) => { sessionId: string | null; sourceId: string | null };
  readonly onEmit?: (event: Pending & { ms: number }) => void;
}

function cachedLetters(surah: number, ayah: number): number {
  const text = reader.versesFor(surah)[ayah - 1];
  if (text === undefined) return FALLBACK_LETTERS;
  return arabicLetterCount(text) || FALLBACK_LETTERS;
}

export class ReadTracker {
  readonly #now: () => number;
  readonly #lettersFor: (surah: number, ayah: number) => number;
  readonly #onRead: NonNullable<ReadTrackerDeps["onRead"]>;
  readonly #onEmit: NonNullable<ReadTrackerDeps["onEmit"]>;
  #watches = new Map<Element, Watch>();
  #lastCounted = new Map<string, number>();
  #pending: Pending[] = [];
  #activeMs = 0;
  #lastTick: number;
  #lastInteraction: number;
  #visible = true;

  constructor(deps: ReadTrackerDeps = {}) {
    this.#now = deps.now ?? (() => Date.now());
    this.#lettersFor = deps.lettersFor ?? cachedLetters;
    this.#onRead =
      deps.onRead ??
      ((surah, ayah) => {
        sessions.noteRead(surah, ayah);
        const thread = sessions.current;
        return { sessionId: thread?.id ?? null, sourceId: thread?.sourceId ?? null };
      });
    this.#onEmit =
      deps.onEmit ??
      ((event) =>
        readingAnalytics.emit({
          sessionId: event.sessionId,
          surah: event.surah,
          ayah: event.ayah,
          sourceId: event.sourceId,
          ms: event.ms,
        }));
    this.#lastTick = this.#now();
    this.#lastInteraction = this.#lastTick;
  }

  /** Accrue active time up to now: only while visible and not idle. */
  #tick(): void {
    const now = this.#now();
    if (this.#visible) {
      const activeUntil = Math.min(now, this.#lastInteraction + IDLE_MS);
      this.#activeMs += Math.max(0, activeUntil - this.#lastTick);
    }
    this.#lastTick = now;
  }

  /** Effective "now" for dwell: an idle reader stops accruing. */
  #dwellNow(): number {
    return Math.min(this.#now(), this.#lastInteraction + IDLE_MS);
  }

  interaction(): void {
    this.#tick();
    this.#lastInteraction = this.#now();
  }

  setVisible(visible: boolean): void {
    this.#tick();
    if (!visible) this.flushVisible();
    this.#visible = visible;
    const now = this.#now();
    if (visible) this.#lastInteraction = now;
    for (const watch of this.#watches.values()) {
      if (!visible && watch.visibleSince !== null) {
        watch.visibleMs += Math.max(0, this.#dwellNow() - watch.visibleSince);
        watch.visibleSince = null;
      } else if (visible && watch.inView && watch.visibleSince === null) {
        // Back on the tab: markers still on screen resume their dwell (the
        // observer never re-fires for them).
        watch.visibleSince = now;
      }
    }
    if (!visible) this.flushWindow();
  }

  track(el: Element, key: string): void {
    const coords = parseVerseKey(key);
    if (coords === null || this.#watches.has(el)) return;
    this.#watches.set(el, { key, ...coords, inView: false, visibleSince: null, visibleMs: 0 });
  }

  untrack(el: Element): void {
    const watch = this.#watches.get(el);
    if (watch === undefined) return;
    // Unmounted while visible = the reader moved on with it on screen.
    if (watch.visibleSince !== null) this.#leave(watch, true);
    this.#watches.delete(el);
  }

  /** Visibility edge for one marker; `above` = it left past the top edge. */
  intersect(el: Element, visible: boolean, above: boolean): void {
    const watch = this.#watches.get(el);
    if (watch === undefined) return;
    watch.inView = visible;
    if (visible) {
      if (watch.visibleSince === null && this.#visible) watch.visibleSince = this.#dwellNow();
      return;
    }
    if (watch.visibleSince === null) return;
    this.#leave(watch, above);
  }

  #leave(watch: Watch, counts: boolean): void {
    if (watch.visibleSince !== null) {
      watch.visibleMs += Math.max(0, this.#dwellNow() - watch.visibleSince);
      watch.visibleSince = null;
    }
    if (counts) this.#maybeCount(watch);
    // Scrolling back (bottom exit) or a counted read both start a fresh dwell.
    watch.visibleMs = 0;
  }

  #maybeCount(watch: Watch): boolean {
    const letters = this.#lettersFor(watch.surah, watch.ayah);
    if (watch.visibleMs < dwellFloorMs(letters)) return false;
    const now = this.#now();
    const last = this.#lastCounted.get(watch.key);
    if (last !== undefined && now - last < REPEAT_GAP_MS) return false;
    this.#lastCounted.set(watch.key, now);
    const context = this.#onRead(watch.surah, watch.ayah);
    if (this.#pending.length === 0) {
      // A new window starts here: time before the first read was browsing.
      this.#tick();
      this.#activeMs = Math.min(this.#activeMs, watch.visibleMs);
    }
    this.#pending.push({ surah: watch.surah, ayah: watch.ayah, letters, ...context });
    return true;
  }

  /**
   * The reader is navigating away / hiding: everything on screen gets its say.
   * A verse not yet over its floor keeps the dwell it has (a hidden tab
   * resumes it); a counted one starts fresh.
   */
  flushVisible(): void {
    for (const watch of this.#watches.values()) {
      if (watch.visibleSince === null) continue;
      watch.visibleMs += Math.max(0, this.#dwellNow() - watch.visibleSince);
      watch.visibleSince = this.#visible ? this.#dwellNow() : null;
      if (this.#maybeCount(watch)) watch.visibleMs = 0;
    }
  }

  /** Emit the pending reads, splitting the window's active time by letters. */
  flushWindow(): void {
    this.#tick();
    const pending = this.#pending;
    if (pending.length === 0) {
      this.#activeMs = 0;
      return;
    }
    const total = pending.reduce((sum, p) => sum + Math.max(1, p.letters), 0);
    const budget = this.#activeMs;
    for (const p of pending) {
      this.#onEmit({ ...p, ms: (budget * Math.max(1, p.letters)) / total });
    }
    this.#pending = [];
    this.#activeMs = 0;
  }

  get pendingCount(): number {
    return this.#pending.length;
  }

  static readonly WINDOW_FLUSH_MS = WINDOW_FLUSH_MS;
}

/**
 * Wire a tracker to the live document: one IntersectionObserver for every
 * verse marker, a MutationObserver for markers the virtual lists mount and
 * unmount, passive interaction listeners, and visibility/pagehide edges.
 * Returns the teardown plus `beforeNavigate` for the layout's router hook.
 */
export interface ReadTrackerHandle {
  /** Call from the router's beforeNavigate: verses on screen get counted first. */
  readonly beforeNavigate: () => void;
  readonly dispose: () => void;
}

export function installReadTracker(tracker: ReadTracker = new ReadTracker()): ReadTrackerHandle {
  const io = new IntersectionObserver((entries) => {
    for (const entry of entries) {
      const rootTop = entry.rootBounds?.top ?? 0;
      tracker.intersect(
        entry.target,
        entry.isIntersecting,
        !entry.isIntersecting && entry.boundingClientRect.bottom <= rootTop,
      );
    }
  });
  const observe = (el: Element): void => {
    if (!(el instanceof HTMLElement)) return;
    const key = el.dataset.verseAnchor;
    if (key === undefined) return;
    tracker.track(el, key);
    io.observe(el);
  };
  const forget = (el: Element): void => {
    io.unobserve(el);
    tracker.untrack(el);
  };
  const scan = (node: Node, fn: (el: Element) => void): void => {
    if (!(node instanceof Element)) return;
    if (node.hasAttribute("data-verse-anchor")) fn(node);
    for (const el of node.querySelectorAll("[data-verse-anchor]")) fn(el);
  };
  const mo = new MutationObserver((records) => {
    for (const record of records) {
      for (const node of record.removedNodes) scan(node, forget);
      for (const node of record.addedNodes) scan(node, observe);
    }
  });
  scan(document.body, observe);
  mo.observe(document.body, { childList: true, subtree: true });

  const onInteract = (): void => tracker.interaction();
  const onVisibility = (): void => tracker.setVisible(document.visibilityState === "visible");
  const onHide = (): void => {
    tracker.flushVisible();
    tracker.flushWindow();
  };
  const timer = setInterval(() => {
    if (tracker.pendingCount > 0) tracker.flushWindow();
  }, WINDOW_FLUSH_MS);
  const opts: AddEventListenerOptions = { passive: true };
  // Activity probes only (no shortcuts): keyboard scrolling surfaces as `scroll`.
  for (const type of ACTIVITY_EVENTS) window.addEventListener(type, onInteract, opts);
  document.addEventListener("visibilitychange", onVisibility);
  window.addEventListener("pagehide", onHide);

  return {
    beforeNavigate: onHide,
    dispose: () => {
      onHide();
      clearInterval(timer);
      io.disconnect();
      mo.disconnect();
      for (const type of ACTIVITY_EVENTS) window.removeEventListener(type, onInteract, opts);
      document.removeEventListener("visibilitychange", onVisibility);
      window.removeEventListener("pagehide", onHide);
    },
  };
}
