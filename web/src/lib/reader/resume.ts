import { goto } from "$app/navigation";

import { loadQuranData } from "#lib/data/quran-data-client.js";
import { resumeCtxFor, surahAyahPathFor, type SurahRouteContext } from "#lib/data/quran.js";
import type { UiLocale } from "#lib/i18n/locales.js";
import { publicHref } from "#lib/i18n/public-href.js";
import { readerHrefFor } from "#lib/i18n/reader.js";
import { getLocale } from "#lib/paraglide/runtime.js";
import { markEntry, type EntryOrigin } from "#lib/sessions/entry.js";
import type { ReadingThread } from "#lib/sessions/schema.js";
import { sessions } from "#lib/sessions/store.svelte.js";
import { reader } from "#lib/stores/reader.svelte.js";

import { threadPath } from "./thread-path";

export { threadPath } from "./thread-path";

export type ResumeOptions = { replaceState?: boolean };

async function go(path: `/${string}`, options: ResumeOptions): Promise<void> {
  // kit 3 merged keepFocus/noScroll into `reset`: it stays true so the reader's
  // own anchor scroll runs; only `replace` varies.
  // SAFETY: paraglide getLocale() returns the active locale, and this app defines exactly the UI_LOCALE_IDS union (en/ar); readerHrefFor re-validates via assertUiLocale.
  await goto(publicHref(readerHrefFor(getLocale() as UiLocale, path)), {
    reset: true,
    replace: options.replaceState,
  });
}

/** Continue a reading thread exactly where it was left (Continue cards, handoff, hotkey). */
export async function resumeThread(
  thread: ReadingThread,
  currentCtx: SurahRouteContext,
  options: ResumeOptions = {},
): Promise<boolean> {
  try {
    const quranData = await loadQuranData();
    const ctx = resumeCtxFor(
      thread.sourceId !== null ? { sourceId: thread.sourceId } : null,
      currentCtx,
    );
    const path = threadPath(thread, ctx, quranData);
    if (path === null) return false;
    sessions.markResume(thread.id);
    reader.openVerse(thread.surah);
    if (thread.localPage !== null && thread.ratio !== null) {
      reader.setPendingAnchor({
        verseKey: `${thread.surah}:${thread.ayah}`,
        localPage: thread.localPage,
        ratio: thread.ratio,
      });
    }
    await go(path, options);
    return true;
  } catch {
    return false;
  }
}

/** Open one verse as a new thread (notes, bookmarks, search hits). */
export async function openVerseAt(
  num: number,
  n: number,
  sourceId: string | undefined,
  currentCtx: SurahRouteContext,
  origin: Exclude<EntryOrigin, "resume"> = "bookmark",
  options: ResumeOptions = {},
): Promise<boolean> {
  try {
    const quranData = await loadQuranData();
    const surah = quranData.surahByNum(num);
    if (!surah || !quranData.surahLocalPageForAyah(num, n)) return false;
    const ctx = resumeCtxFor(sourceId !== undefined ? { sourceId } : null, currentCtx);
    markEntry({ origin, exact: true });
    reader.openVerse(num);
    await go(surahAyahPathFor(ctx, surah, n), options);
    return true;
  } catch {
    return false;
  }
}

/** "Continue reading": this tab's thread, else the newest open one. */
export async function resumeToLastRead(
  currentCtx: SurahRouteContext,
  options: ResumeOptions = {},
): Promise<boolean> {
  const thread = sessions.current ?? sessions.latest;
  if (thread === null) return false;
  return resumeThread(thread, currentCtx, options);
}
