const INVALID_SELECTOR = "[data-invalid='true'], [aria-invalid='true']";

/** Focus the first invalid field within `root`. Returns true if a field was focused. */
export function focusFirstInvalid(root: ParentNode): boolean {
  const errored = root.querySelector<HTMLElement>(INVALID_SELECTOR);
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- capability probe on an arbitrary matched element (selector can hit any element kind); typeof is the only cross-realm-safe function check and no parse boundary exists here
  if (!errored || typeof errored.focus !== "function") return false;
  errored.focus();
  return true;
}
