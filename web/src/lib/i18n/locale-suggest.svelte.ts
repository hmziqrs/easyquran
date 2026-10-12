import { authModal } from "#lib/auth/auth-modal.svelte.js";
import { track } from "#lib/firebase/analytics.js";
import { SUPPORTED_UI_LOCALES, isUiLocale, type UiLocale } from "#lib/i18n/locales.js";
import { publicHref } from "#lib/i18n/public-href.js";
import { isReaderSurfacePath } from "#lib/i18n/reader.js";
import {
  deLocalizeHref,
  extractLocaleFromNavigator,
  localizeHref,
} from "#lib/paraglide/runtime.js";
import { commandPalette } from "#lib/stores/command-palette.svelte.js";
import {
  asLiteral,
  asObject,
  isFutureSchema,
  onStorageKey,
  readJSON,
  writeJSON,
} from "#lib/storage/index.js";

// Locale-suggest popup state (plan 09 / next-features Feature 10). Client-side only: the URL stays the sole locale truth; accept is a full-reload link, never a redirect or cookie. One localStorage key, write-through over an in-memory `resolved` map so blocked storage degrades to per-page-load.
export const LOCALE_SUGGEST_STORAGE_KEY = "easyquran.localeSuggest";

const SCHEMA_VERSION = 1;
const RESOLVED_VALUES = ["accepted", "dismissed"] as const;

export type LocaleSuggestResolution = (typeof RESOLVED_VALUES)[number];
export type LocaleSuggestDismissReason = "escape" | "overlay" | "button" | "close_icon";
type ResolvedMap = Partial<Record<UiLocale, LocaleSuggestResolution>>;

// Bounded app-chrome pages with `/ar` twins via the hooks.ts reroute (scheme A).
const APP_CHROME_SEGMENTS = new Set(["search", "settings", "bookmarks", "yours"]);

// Plan §3 row 5, scheme A: only the marketing home and the reader/app surfaces have
// localized twins. Marketing subpages, auth, account, and content exports have no
// `/ar` variant — no popup there, and nothing recorded (the one chance is not burned).
export function isLocaleSuggestSurface(pathname: string): boolean {
  if (pathname === "/" || pathname === "/ar" || pathname === "/ar/") return true;
  const delocalized = deLocalizeHref(pathname);
  if (APP_CHROME_SEGMENTS.has(delocalized.split("/")[1] ?? "")) return true;
  return isReaderSurfacePath(delocalized);
}

/** Accept-link href for `target`, or "" when no safe twin exists (off-site or same path). */
export function buildTargetHref(
  pathname: string,
  search: string,
  hash: string,
  target: UiLocale,
): string {
  // URLPattern("/ar/") does not match "/ar", so deLocalizeHref would leave it unstripped — normalize first (§2.3).
  const normalized = pathname === "/ar" ? "/ar/" : pathname;
  const localizedPath = localizeHref(deLocalizeHref(normalized), { locale: target });
  if (!localizedPath.startsWith("/") || localizedPath.startsWith("//")) return "";
  // SAFETY: the checks above proved localizedPath is a same-origin absolute path, the shape publicHref requires.
  const href = `${publicHref(localizedPath as `/${string}`)}${search}${hash}`;
  if (href === `${normalized}${search}${hash}`) return "";
  return href;
}

// Tolerant decode: future versions, junk values, and unknown locales drop out.
// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is arbitrary JSON.parse output of the localStorage payload; this decoder is the boundary parser (asObject + literal-union validation), mirroring asBooleanRecord in lib/storage/decoders.ts.
export function decodeResolved(raw: unknown) {
  const out: ResolvedMap = {};
  if (isFutureSchema(raw, SCHEMA_VERSION)) return out;
  const resolved = asObject(asObject(raw)?.resolved);
  if (!resolved) return out;
  for (const locale of SUPPORTED_UI_LOCALES) {
    const value = asLiteral(resolved[locale], RESOLVED_VALUES);
    if (value !== undefined) out[locale] = value;
  }
  return out;
}

export class LocaleSuggestStore {
  open = $state(false);
  target = $state<UiLocale | null>(null);
  current = $state<UiLocale>("en");
  href = $state("/");

  private resolved: ResolvedMap = {};
  private shownThisLoad = false;
  private retriesLeft = 1;
  private retryArmed = false;
  private acceptedThisOpen = false;
  private pendingReason: LocaleSuggestDismissReason | null = null;

  hydrate(): void {
    this.resolved = decodeResolved(readJSON(LOCALE_SUGGEST_STORAGE_KEY));
  }
  // Cross-tab: another tab's answer stands; its open dialog closes without writing.
  subscribeExternal(): () => void {
    return onStorageKey(LOCALE_SUGGEST_STORAGE_KEY, () => this.onExternalStorage());
  }

  private onExternalStorage(): void {
    const external = decodeResolved(readJSON(LOCALE_SUGGEST_STORAGE_KEY));
    this.resolved = { ...this.resolved, ...external };
    if (this.open && this.target !== null && external[this.target] !== undefined) this.forceClose();
  }

  evaluate(pathname: string, search = location.search, hash = location.hash): void {
    if (this.shownThisLoad) return;
    if (!isLocaleSuggestSurface(pathname)) return;
    const suggested = extractLocaleFromNavigator();
    // Never getLocale(): prefix-less URLs resolve to `en` without the middleware (app-locale.ts).
    const current = pathname === "/ar" || pathname.startsWith("/ar/") ? "ar" : "en";
    if (!isUiLocale(suggested) || suggested === current) return;
    if (this.resolved[suggested] !== undefined) return;
    const href = buildTargetHref(pathname, search, hash, suggested);
    if (href === "") return;
    if (authModal.open || commandPalette.open) {
      if (this.retriesLeft > 0) {
        this.retriesLeft -= 1;
        this.retryArmed = true;
      }
      return;
    }
    this.target = suggested;
    this.current = current;
    this.href = href;
    this.pendingReason = null;
    this.acceptedThisOpen = false;
    this.open = true;
    this.shownThisLoad = true;
    this.retryArmed = false;
    void track("locale_suggest_shown", { suggested_locale: suggested });
  }

  // Fired by the modal guard when auth/palette close: one retry per page load.
  retryIfArmed(pathname: string): void {
    if (!this.retryArmed || this.shownThisLoad) return;
    this.retryArmed = false;
    this.evaluate(pathname);
  }

  // Condition 6b: a modal opened on top — close, record nothing, latch.
  forceClose(): void {
    if (!this.open) return;
    this.open = false;
    this.target = null;
    this.shownThisLoad = true;
  }

  // Noted before bits-ui's onOpenChange(false) arrives with no reason attached.
  noteCloseIntent(reason: LocaleSuggestDismissReason): void {
    this.pendingReason = reason;
  }

  // The single bits-ui close funnel — dismissal written exactly once (open guard = no-op repeats).
  onDialogOpenChange(open: boolean): void {
    if (open) {
      this.open = true;
      return;
    }
    this.dismiss(this.pendingReason ?? "close_icon");
  }

  // Accept persists and lets the accept link navigate (plan §5.4): the dialog stays
  // mounted so the full-reload anchor is never torn down mid-click. If that reload
  // somehow fails, the dialog remains dismissable like any open dialog.
  accept(): void {
    const target = this.target;
    if (!this.open || target === null || this.acceptedThisOpen) return;
    this.acceptedThisOpen = true;
    this.resolve(target, "accepted");
    void track("locale_suggest_accepted", { suggested_locale: target });
  }

  dismiss(reason: LocaleSuggestDismissReason): void {
    const target = this.target;
    if (!this.open || target === null) return;
    this.resolve(target, "dismissed");
    void track("locale_suggest_dismissed", { suggested_locale: target, dismiss_reason: reason });
    this.open = false;
    this.target = null;
  }

  private resolve(locale: UiLocale, value: LocaleSuggestResolution): void {
    const next: ResolvedMap = {};
    for (const key of SUPPORTED_UI_LOCALES) {
      const existing = this.resolved[key];
      if (existing !== undefined) next[key] = existing;
    }
    next[locale] = value;
    this.resolved = next;
    writeJSON(LOCALE_SUGGEST_STORAGE_KEY, { v: SCHEMA_VERSION, resolved: next });
  }
}

export const localeSuggest = new LocaleSuggestStore();
