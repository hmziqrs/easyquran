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
