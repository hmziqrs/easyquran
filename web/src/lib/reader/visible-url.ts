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
