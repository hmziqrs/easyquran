const INVALID_SELECTOR = "[data-invalid='true'], [aria-invalid='true']";

/** Focus the first invalid field within `root`. Returns true if a field was focused. */
export function focusFirstInvalid(root: ParentNode): boolean {
  const errored = root.querySelector<HTMLElement>(INVALID_SELECTOR);
  if (!errored || typeof errored.focus !== "function") return false;
  errored.focus();
  return true;
}
