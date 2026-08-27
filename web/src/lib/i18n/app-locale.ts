import { getContext, setContext } from "svelte";

import { DEFAULT_UI_LOCALE, type UiLocale } from "$lib/i18n/locales";

/**
 * Locale hand-off for the de-localized app pages (`/app/bookmarks`, and the
 * settings/search precedent routes). Those URLs carry no `/{en,ar}` prefix, so
 * paraglide's url strategy resolves them to the base locale — on the server
 * hooks.server.ts never runs the paraglide middleware for them, and on the
 * client `getLocale()` re-resolves from the current URL, which has already lost
 * the prefix by the time the page component mounts after a client-side
 * navigation away from a localized reader route.
 *
 * The app layout mounts while the URL still carries the locale (locale
 * switching is a full reload via `data-sveltekit-reload`), so it publishes its
 * `copy.locale` here; de-localized pages read it back instead of calling
 * `getLocale()` themselves. Chrome (nav/footer) already works this way
 * implicitly — the layout hands its resolved copy down as props, which is why
 * the nav stayed Arabic while a page resolving its own copy fell back to
 * English. Same key shape as the ui/sidebar context precedent.
 */
const APP_LOCALE_KEY = "easyquran.app-locale";

/** Publish the reader app shell locale for descendant pages. Layout-init time only. */
export function setAppLocale(locale: UiLocale): void {
  setContext(Symbol.for(APP_LOCALE_KEY), locale);
}

/**
 * Active app-shell locale for a de-localized page. Falls back to the base
 * locale when no layout provided one (isolated mounts, tests) — identical to
 * what `getLocale()` resolves on these prefix-less URLs.
 */
export function appLocale(fallback: UiLocale = DEFAULT_UI_LOCALE): UiLocale {
  return getContext<UiLocale | undefined>(Symbol.for(APP_LOCALE_KEY)) ?? fallback;
}
