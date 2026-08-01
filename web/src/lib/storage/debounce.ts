/* ════════════════════════════════════════════════════════════════════════
   debounce.ts — a minimal trailing-edge debounce with explicit flush/cancel.

   Used by the reader's annotations facet so a note that changes on every
   keystroke doesn't hit (synchronous) localStorage on every keystroke. The
   domain owns scheduling (delay, when to flush on close / page-hide); this
   helper owns only the timer mechanic. No framework dependency, so it is
   unit-testable with fake timers in isolation.
   ════════════════════════════════════════════════════════════════════════ */

export interface Debounced {
  schedule(): void;
  flush(): void;
  cancel(): void;
  readonly pending: boolean;
}

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
