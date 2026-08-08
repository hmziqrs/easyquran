import type { SurahLocalPageData } from "$lib/data/quran";
import { asObject, readRaw, writeRaw } from "$lib/storage";
import type { ViewportAnchor } from "./viewport-anchor";

export interface SurahReaderHistoryState {
  version: 1;
  surahNum: number;
  activeLocalPage: number;
  pages: SurahLocalPageData[];
  anchor: ViewportAnchor | null;
}

const readerPositionKey = "easyquran.reader-position";

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

function validateAnchor(raw: unknown): ViewportAnchor | null {
  const obj = asObject(raw);
  if (!obj) return null;
  const { localPage, ratio, viewportPoint } = obj;
  if (
    typeof localPage !== "number" ||
    !Number.isSafeInteger(localPage) ||
    typeof ratio !== "number" ||
    !Number.isFinite(ratio) ||
    typeof viewportPoint !== "number" ||
    !Number.isFinite(viewportPoint)
  ) {
    return null;
  }
  if (obj.kind === "verse") {
    if (typeof obj.verseKey !== "string") return null;
    return { kind: "verse", localPage, verseKey: obj.verseKey, viewportPoint, ratio };
  }
  if (obj.kind === "page") {
    return { kind: "page", localPage, viewportPoint, ratio };
  }
  return null;
}

export function parseHistoryState(
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
    const pages = state.pages.filter(
      (p): p is SurahLocalPageData =>
        p != null && typeof p === "object" && Number.isSafeInteger(p.page?.localPage),
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
  const navigation = performance.getEntriesByType("navigation")[0] as
    | PerformanceNavigationTiming
    | undefined;
  if (navigation?.type !== "reload") return null;
  let state: Partial<SurahReaderHistoryState> | null = null;
  try {
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
