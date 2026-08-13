import { goto } from "$app/navigation";
import { resumeCtxFor, surahAyahPathFor, type SurahRouteContext } from "$lib/data/quran";
import { loadQuranData } from "$lib/data/quran-data-client";
import type { UiLocale } from "$lib/i18n/locales";
import { publicHref } from "$lib/i18n/public-href";
import { readerHrefFor } from "$lib/i18n/reader";
import { getLocale } from "$lib/paraglide/runtime.js";
import type { LastReadAnchor } from "$lib/stores/reader-core.svelte";
import { reader } from "$lib/stores/reader.svelte";

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
    const targetPage = quranData.surahLocalPageForAyah(num, n);
    if (!surah || !targetPage) return false;
    const resumeCtx = resumeCtxFor(sourceId !== undefined ? { sourceId } : null, currentCtx);
    reader.openVerse(num, n, sourceId);
    if (options.anchor) reader.setPendingAnchor(options.anchor);
    // SAFETY: paraglide getLocale() returns the active locale, and this app defines exactly the UI_LOCALE_IDS union (en/ar); readerHrefFor re-validates via assertUiLocale.
    await goto(
      publicHref(
        readerHrefFor(
          getLocale() as UiLocale,
          surahAyahPathFor(resumeCtx, surah, targetPage.localPage, n),
        ),
      ),
      {
        keepFocus: !options.replaceState,
        replaceState: options.replaceState,
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
