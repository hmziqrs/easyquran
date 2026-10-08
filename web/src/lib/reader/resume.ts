import { goto } from "$app/navigation";

import { loadQuranData } from "#lib/data/quran-data-client.js";
import { resumeCtxFor, surahAyahPathFor, type SurahRouteContext } from "#lib/data/quran.js";
import type { UiLocale } from "#lib/i18n/locales.js";
import { publicHref } from "#lib/i18n/public-href.js";
import { readerHrefFor } from "#lib/i18n/reader.js";
import { getLocale } from "#lib/paraglide/runtime.js";
import type { LastReadAnchor } from "#lib/stores/reader-core.svelte.js";
import { reader } from "#lib/stores/reader.svelte.js";

export type ResumeOptions = { replaceState?: boolean; anchor?: LastReadAnchor | null };

export async function resumeToVerse(
  num: number,
  n: number,
  sourceId: string | undefined,
  currentCtx: SurahRouteContext,
  options: ResumeOptions = {},
): Promise<boolean> {
  try {
    const quranData = await loadQuranData();
    const surah = quranData.surahByNum(num);
    if (!surah || !quranData.surahLocalPageForAyah(num, n)) return false;
    const resumeCtx = resumeCtxFor(sourceId !== undefined ? { sourceId } : null, currentCtx);
    reader.openVerse(num, n, sourceId);
    if (options.anchor) reader.setPendingAnchor(options.anchor);
    // SAFETY: paraglide getLocale() returns the active locale, and this app defines exactly the UI_LOCALE_IDS union (en/ar); readerHrefFor re-validates via assertUiLocale.
    // kit 3 merged keepFocus/noScroll into one `reset` flag. Kit 2 passed
    // keepFocus: !replaceState, and every live caller omits options — so focus
    // was always KEPT while scroll still reset to the pending anchor. Kit 3
    // cannot express that split (reset: false would also skip the anchor
    // scroll), so reset stays true: anchor scrolling is preserved and the
    // focus reset is an accepted, documented drift; only `replace` varies.
    await goto(
      publicHref(readerHrefFor(getLocale() as UiLocale, surahAyahPathFor(resumeCtx, surah, n))),
      {
        reset: true,
        replace: options.replaceState,
      },
    );
    return true;
  } catch {
    return false;
  }
}

export async function resumeToLastRead(
  currentCtx: SurahRouteContext,
  options: ResumeOptions = {},
): Promise<boolean> {
  const lastRead = reader.lastRead;
  if (!lastRead) return false;
  return resumeToVerse(lastRead.num, lastRead.n, lastRead.sourceId, currentCtx, {
    ...options,
    anchor: reader.lastReadAnchor,
  });
}
