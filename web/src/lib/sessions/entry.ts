import { asLiteral } from "#lib/storage/index.js";

import { SESSION_ORIGINS, isScope, parseVerseKey, type SessionOrigin } from "./schema";

/**
 * Entry tagging (docs/reading-model.md §2.1). A navigation INTO the reader
 * from somewhere else carries an intent: links declare it with
 * `data-session-*` attributes (one capture-phase click listener reads them),
 * programmatic jumps call `markEntry()` right before their `goto`. The reader
 * consumes the intent when it resolves its thread. Untagged in-reader
 * navigation (next ayah/page/surah, reload) is continuation.
 */

export type EntryOrigin = SessionOrigin | "resume";

export interface EntryIntent {
  readonly origin: EntryOrigin;
  /** Origin detail, e.g. the card id. */
  readonly ref?: string | null;
  /** Overrides the reader's natural scope (card ranges, snippet sets). */
  readonly scope?: string | null;
  readonly boundEnd?: string | null;
  /** The target names an exact ayah (search hit, bookmark) — always a new thread. */
  readonly exact?: boolean;
  /** `resume` only: the thread to continue. */
  readonly threadId?: string;
}

/** An intent older than this is stale (the navigation it tagged never landed). */
const INTENT_TTL_MS = 15_000;

let pending: (EntryIntent & { readonly at: number }) | null = null;
let leftReader = false;

export function markEntry(intent: EntryIntent): void {
  pending = { ...intent, at: Date.now() };
}

/** The pending intent, once — a second read returns null. */
export function consumeEntry(nowMs: number = Date.now()): EntryIntent | null {
  const intent = pending;
  pending = null;
  if (intent === null || nowMs - intent.at > INTENT_TTL_MS) return null;
  return intent;
}

/** Recorded on every client navigation: was the page being left a reader page? */
export function noteLeaving(fromReader: boolean): void {
  leftReader = fromReader;
}

export function cameFromReader(): boolean {
  return leftReader;
}

/** The `data-session-*` attributes a reader-entry link carries. */
export interface EntryLinkAttrs {
  "data-session-origin": string;
  "data-session-ref"?: string;
  "data-session-scope"?: string;
  "data-session-bound"?: string;
  "data-session-exact"?: "true";
}

/** Markup helper: spread onto an `<a>` that enters the reader. */
export function entryAttrs(intent: Omit<EntryIntent, "threadId">): EntryLinkAttrs {
  const attrs: EntryLinkAttrs = { "data-session-origin": intent.origin };
  if (intent.ref) attrs["data-session-ref"] = intent.ref;
  if (intent.scope) attrs["data-session-scope"] = intent.scope;
  if (intent.boundEnd) attrs["data-session-bound"] = intent.boundEnd;
  if (intent.exact === true) attrs["data-session-exact"] = "true";
  return attrs;
}

/** Parse a tagged link back into an intent; anything malformed is ignored. */
export function intentFromElement(el: HTMLElement): EntryIntent | null {
  const origin = asLiteral<SessionOrigin>(el.dataset.sessionOrigin, SESSION_ORIGINS);
  if (origin === undefined) return null;
  const scope = el.dataset.sessionScope ?? null;
  const bound = el.dataset.sessionBound ?? null;
  return {
    origin,
    ref: el.dataset.sessionRef ?? null,
    scope: scope !== null && isScope(scope) ? scope : null,
    boundEnd: bound !== null && parseVerseKey(bound) !== null ? bound : null,
    exact: el.dataset.sessionExact === "true",
  };
}

/**
 * One capture-phase listener for the whole app: a plain left click on a
 * tagged link marks the entry before SvelteKit's router handles it. Modified
 * clicks open a new tab, which starts its own thread anyway.
 */
export function installEntryTagging(): () => void {
  const onClick = (event: MouseEvent): void => {
    if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) {
      return;
    }
    const target = event.target;
    if (!(target instanceof Element)) return;
    const link = target.closest<HTMLElement>("[data-session-origin]");
    if (link === null) return;
    const intent = intentFromElement(link);
    if (intent !== null) markEntry(intent);
  };
  document.addEventListener("click", onClick, true);
  return () => document.removeEventListener("click", onClick, true);
}

const READER_ROUTE_RE =
  /\/(?:\[surah\]|page\/\[n\]|juz\/\[n\]|hizb\/\[n\]|azkar\/\[slug\])(?:\/|$)/u;

/** Reader route ids (surah, page, juz, hizb, snippet — Arabic and `/t/**` twins). */
export function isReaderRouteId(routeId: string | null | undefined): boolean {
  return routeId !== null && routeId !== undefined && READER_ROUTE_RE.test(routeId);
}
