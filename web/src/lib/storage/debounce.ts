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
