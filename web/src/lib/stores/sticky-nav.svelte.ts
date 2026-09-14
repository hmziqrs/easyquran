class StickyNav {
  collapsed = $state(false);
  readonly height = 56;
  #suppressDepth = 0;
  #suppressUntil = 0;

  collapse(): void {
    this.collapsed = true;
  }
  expand(): void {
    this.collapsed = false;
  }

  /**
   * True while — and briefly after — the reader performs programmatic scrolls
   * (anchor restores, spacer swaps). Those scrolls move window.scrollY without
   * user intent, so the collapse/expand direction logic must ignore them: it
   * only resyncs its baseline. This is the header-flash fix.
   */
  get scrollSuppressed(): boolean {
    return this.#suppressDepth > 0 || Date.now() < this.#suppressUntil;
  }

  /**
   * Mark a programmatic-scroll window. The returned release function is
   * idempotent. The suppression outlives the window by a short tail so scroll
   * events and queued rAF reads that land after the restore still see it.
   */
  suppressProgrammaticScroll(): () => void {
    this.#suppressDepth += 1;
    this.#extendSuppression();
    let released = false;
    return () => {
      if (released) return;
      released = true;
      this.#suppressDepth -= 1;
      this.#extendSuppression();
    };
  }

  #extendSuppression(): void {
    this.#suppressUntil = Date.now() + 250;
  }
}

export const stickyNav = new StickyNav();
