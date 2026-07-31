/* ════════════════════════════════════════════════════════════════════════
   debounce.ts — a minimal trailing-edge debounce with explicit flush/cancel.

   Used by the reader's annotations facet so a note that changes on every
   keystroke doesn't hit (synchronous) localStorage on every keystroke. The
   domain owns scheduling (delay, when to flush on close / page-hide); this
   helper owns only the timer mechanic. No framework dependency, so it is
   unit-testable with fake timers in isolation.
   ════════════════════════════════════════════════════════════════════════ */

export interface Debounced {
  /** Schedule (or reschedule) a trailing invocation. */
  schedule(): void;
  /** Invoke immediately and clear any pending timer. */
  flush(): void;
  /** Drop the pending invocation without running it. */
  cancel(): void;
  /** Whether a trailing invocation is currently pending. */
  readonly pending: boolean;
}

/**
 * Create a trailing-edge debounce around `fn`. At most one invocation runs
 * `wait` ms after the last `schedule()`. `flush()` runs it immediately (used
 * on note-close / page-hide); `cancel()` discards a pending run.
 */
export function trailingDebounce(fn: () => void, wait: number): Debounced {
  let timer: ReturnType<typeof setTimeout> | null = null;

  const run = (): void => {
    timer = null;
    fn();
  };

  return {
    schedule(): void {
      if (timer !== null) clearTimeout(timer);
      timer = setTimeout(run, wait);
    },
    flush(): void {
      if (timer !== null) {
        clearTimeout(timer);
        run();
      }
    },
    cancel(): void {
      if (timer !== null) {
        clearTimeout(timer);
        timer = null;
      }
    },
    get pending(): boolean {
      return timer !== null;
    },
  };
}
