import type { SurahLocalPageData } from "$lib/data/quran";
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
  try {
    sessionStorage.setItem(
      readerPositionKey,
      JSON.stringify({
        version: snapshot.version,
        surahNum: snapshot.surahNum,
        activeLocalPage: snapshot.activeLocalPage,
        anchor: snapshot.anchor,
      }),
    );
  } catch {}
}

export function parseHistoryState(
  value: unknown,
  surahNum: number,
): SurahReaderHistoryState | null {
  if (!value || typeof value !== "object") return null;
  const state = value as Partial<SurahReaderHistoryState>;
  if (
    state.version === 1 &&
    state.surahNum === surahNum &&
    state.activeLocalPage === currentUrlLocalPage() &&
    Array.isArray(state.pages) &&
    state.pages.some((pageData) => pageData?.page?.localPage === state.activeLocalPage)
  ) {
    return state as SurahReaderHistoryState;
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
  try {
    const state = JSON.parse(
      sessionStorage.getItem(readerPositionKey) ?? "null",
    ) as Partial<SurahReaderHistoryState> | null;
    if (
      state?.version === 1 &&
      state.surahNum === initial.surah.num &&
      state.activeLocalPage === initial.page.localPage &&
      state.anchor
    ) {
      return {
        version: 1,
        surahNum: initial.surah.num,
        activeLocalPage: initial.page.localPage,
        pages: [initial],
        anchor: state.anchor,
      };
    }
  } catch {}
  return null;
}
