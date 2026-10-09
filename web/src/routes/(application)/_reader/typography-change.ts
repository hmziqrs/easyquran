import { tick } from "svelte";
import { stickyNav } from "#lib/stores/sticky-nav.svelte.js";

/**
 * A−/A+ live in the ReaderShell sticky bar, but only the mounted reader knows how to keep
 * the reading position across a reflow. The reader registers its anchor-preserving wrapper
 * here; the bar routes size changes through it (plain change when no reader is mounted).
 */
type TypographyWrapper = (change: () => void) => void;

let wrapper: TypographyWrapper | null = null;

/** Registers the reader's wrapper; returns the unregister function for its cleanup. */
export function registerTypographyWrapper(next: TypographyWrapper): () => void {
  wrapper = next;
  return () => {
    if (wrapper === next) wrapper = null;
  };
}

export function changeTypography(change: () => void): void {
  if (wrapper) wrapper(change);
  else change();
}

/** Resolves after the browser has laid out and painted the current DOM. */
function nextFrame(): Promise<void> {
  return new Promise((resolve) => requestAnimationFrame(() => resolve()));
}

/** The first ayah whose box reaches below `top` — what the reader is looking at. */
function firstVisibleVerse(top: number): HTMLElement | null {
  for (const el of document.querySelectorAll<HTMLElement>("[data-verse-key]")) {
    if (el.getBoundingClientRect().bottom > top) return el;
  }
  return null;
}

/**
 * A reader-agnostic wrapper for readers without a virtual list (the range readers): keep the
 * first visible ayah at the same screen offset across a reflow (mode switch, A−/A+). At the
 * very top nothing above can reflow, so the page simply stays at the top.
 */
export function keepVisibleVerse(headerOffset: () => number): TypographyWrapper {
  return (change) => {
    const top = headerOffset();
    const anchor = window.scrollY > 0 ? firstVisibleVerse(top) : null;
    const key = anchor?.dataset.verseKey ?? null;
    const before = anchor?.getBoundingClientRect().top ?? 0;
    const release = stickyNav.suppressProgrammaticScroll();
    change();
    void tick()
      .then(nextFrame)
      .then(() => {
        if (key === null) return;
        const after = document
          .querySelector<HTMLElement>(`[data-verse-key="${CSS.escape(key)}"]`)
          ?.getBoundingClientRect().top;
        if (after !== undefined) window.scrollBy(0, after - before);
      })
      .finally(release);
  };
}
