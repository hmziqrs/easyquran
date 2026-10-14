import type { Page, ReadonlyURL } from "$app/state";

/**
 * The URL the user sees. Kit 3 shallow routing (`goto` with `shallow: true`,
 * which replaced the deprecated `replaceState`/`pushState`) no longer updates
 * `page.url` — the target moves to `page.shallow.url` while `page.url` keeps
 * describing the rendered page
 * (svelte.dev/docs/kit/shallow-routing). URL-sync code must read this and never
 * `page.url` directly, or it re-fires forever chasing a write it can never observe
 * (an unbounded shallow-write loop that ends with the renderer being killed).
 */
export function visibleUrl(page: Page): ReadonlyURL {
  return page.shallow?.url ?? page.url;
}

/**
 * True while the address bar still shows the rendered page's path. Kit 3's
 * shallow `goto` (update_state) replaces the history entry the browser sits on
 * with no pathname check, and during a popstate traversal `window.location` is
 * already the destination while `page` still describes the rendered route —
 * the exact window in which a URL-sync write would clobber the traversed-to
 * entry's URL and pageUrl metadata. Client-only: reads `window.location`, so
 * every caller must run browser-side.
 */
export function locationMatchesVisibleUrl(page: Page): boolean {
  return window.location.pathname === visibleUrl(page).pathname;
}
