import type { Page, ReadonlyURL } from "$app/state";

/**
 * The URL the user sees. Kit 3 shallow routing (`replaceState`/`pushState`, or `goto`
 * with `shallow: true`) no longer updates `page.url` — the target moves to
 * `page.shallow.url` while `page.url` keeps describing the rendered page
 * (svelte.dev/docs/kit/shallow-routing). URL-sync code must read this and never
 * `page.url` directly, or it re-fires forever chasing a write it can never observe
 * (an unbounded replaceState loop that ends with the renderer being killed).
 */
export function visibleUrl(page: Page): ReadonlyURL {
  return page.shallow?.url ?? page.url;
}
