import type { SurahLocalPageData } from "$lib/data/quran";
import { asObject, readRaw, writeRaw } from "$lib/storage";
import type { JsonRecord } from "$lib/storage/decoders";

import type { ViewportAnchor } from "./viewport-anchor";

export interface SurahReaderHistoryState {
  version: 1;
  surahNum: number;
  activeLocalPage: number;
  pages: SurahLocalPageData[];
  anchor: ViewportAnchor | null;
}

const readerPositionKey = "easyquran.reader-position";

// eslint-disable-next-line anti-slop/no-unknown-parameters -- type-guard helper proving number-ness of unvalidated JSON anchor fields; unknown is the honest input domain.
function isNumber(v: unknown): v is number {
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- number-ness of unvalidated JSON fields is the invariant being proved; no parser exists upstream.
  return typeof v === "number";
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- guard for untyped JSON values arriving from history.state / sessionStorage; unknown is the honest input domain.
function isJsonObject(v: unknown): v is JsonRecord {
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- JSON payloads carry no runtime schema; typeof-object is the only discriminator at this parse boundary.
  return v !== null && typeof v === "object" && !Array.isArray(v);
}

export function currentUrlLocalPage(): number {
  const match = /\/page\/(\d+)\/?$/.exec(window.location.pathname);
  return match ? Number(match[1]) : 1;
}

export function persistReaderPosition(snapshot: SurahReaderHistoryState): void {
  writeRaw(
    "session",
    readerPositionKey,
    JSON.stringify({
      version: snapshot.version,
      surahNum: snapshot.surahNum,
      activeLocalPage: snapshot.activeLocalPage,
      anchor: snapshot.anchor,
    }),
  );
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- this fn IS the boundary parser: raw is an unvalidated anchor read from history.state / sessionStorage JSON.
function validateAnchor(raw: unknown): ViewportAnchor | null {
  const obj = asObject(raw);
  if (!obj) return null;
  const { localPage, ratio, viewportPoint } = obj;
  if (
    !isNumber(localPage) ||
    !Number.isSafeInteger(localPage) ||
    !isNumber(ratio) ||
    !Number.isFinite(ratio) ||
    !isNumber(viewportPoint) ||
    !Number.isFinite(viewportPoint)
  ) {
    return null;
  }
  if (obj.kind === "verse") {
    // eslint-disable-next-line anti-slop/no-runtime-typeof -- verseKey is an unvalidated JSON field; string-ness is the discriminant being proved.
    if (typeof obj.verseKey !== "string") return null;
    return { kind: "verse", localPage, verseKey: obj.verseKey, viewportPoint, ratio };
  }
  if (obj.kind === "page") {
    return { kind: "page", localPage, viewportPoint, ratio };
  }
  return null;
}

export function parseHistoryState(
  // eslint-disable-next-line anti-slop/no-unknown-parameters -- value is history.state.surahReader, untyped by SvelteKit (Page.state is a plain record); this fn is the parser.
  value: unknown,
  surahNum: number,
): SurahReaderHistoryState | null {
  const state = asObject(value);
  if (!state) return null;
  const activeLocalPage = currentUrlLocalPage();
  if (
    state.version === 1 &&
    state.surahNum === surahNum &&
    state.activeLocalPage === activeLocalPage &&
    Array.isArray(state.pages)
  ) {
    // SAFETY: Page.state is untyped at runtime (SvelteKit keeps it a plain record); every element
    // is proved a JSON object with a safe-integer page.localPage by the filter predicate below.
    const rawPages = state.pages as unknown[];
    const pages = rawPages.filter(
      (p): p is SurahLocalPageData =>
        isJsonObject(p) && isJsonObject(p.page) && Number.isSafeInteger(p.page.localPage),
    );
    if (pages.some((pageData) => pageData.page.localPage === activeLocalPage)) {
      return {
        version: 1,
        surahNum,
        activeLocalPage,
        pages,
        anchor: validateAnchor(state.anchor),
      };
    }
  }
  return null;
}

/**
 * Position saved to sessionStorage, used only across a full reload (history
 * state does not survive one).
 */
export function reloadPositionState(initial: SurahLocalPageData): SurahReaderHistoryState | null {
  // SAFETY: getEntriesByType("navigation") yields only PerformanceNavigationTiming entries per the
  // Performance Timeline spec; index 0 may be absent, hence the undefined union.
  const navigation = performance.getEntriesByType("navigation")[0] as
    | PerformanceNavigationTiming
    | undefined;
  if (navigation?.type !== "reload") return null;
  let state: Partial<SurahReaderHistoryState> | null = null;
  try {
    // SAFETY: payload is our own persistReaderPosition JSON; every field is re-checked below
    // (version === 1, surahNum/activeLocalPage equality, validateAnchor) before any use.
    state = JSON.parse(
      readRaw("session", readerPositionKey) ?? "null",
    ) as Partial<SurahReaderHistoryState> | null;
  } catch {
    return null;
  }
  const anchor = validateAnchor(state?.anchor);
  if (
    state?.version === 1 &&
    state.surahNum === initial.surah.num &&
    state.activeLocalPage === initial.page.localPage &&
    anchor
  ) {
    return {
      version: 1,
      surahNum: initial.surah.num,
      activeLocalPage: initial.page.localPage,
      pages: [initial],
      anchor,
    };
  }
  return null;
}
