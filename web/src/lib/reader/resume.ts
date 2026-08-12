import { goto } from "$app/navigation";
import { resumeCtxFor, surahAyahPathFor, type SurahRouteContext } from "$lib/data/quran";
import { loadQuranData } from "$lib/data/quran-data-client";
import { readerHrefFor } from "$lib/i18n/reader";
import { publicHref } from "$lib/i18n/public-href";
import { getLocale } from "$lib/paraglide/runtime.js";
import type { UiLocale } from "$lib/i18n/locales";
import { reader } from "$lib/stores/reader.svelte";

export type ResumeOptions = { replaceState?: boolean };

export async function resumeToLastRead(
  currentCtx: SurahRouteContext,
  options: ResumeOptions = {},
): Promise<boolean> {
  const lastRead = reader.lastRead;
  if (!lastRead) return false;
  const anchor = reader.lastReadAnchor;
  try {
    const quranData = await loadQuranData();
    const surah = quranData.surahByNum(lastRead.num);
    const targetPage = quranData.surahLocalPageForAyah(lastRead.num, lastRead.n);
    if (!surah || !targetPage) return false;
    const resumeCtx = resumeCtxFor(lastRead, currentCtx);
    reader.openVerse(lastRead.num, lastRead.n, lastRead.sourceId);
    if (anchor) reader.setPendingAnchor(anchor);
    await goto(
      publicHref(
        readerHrefFor(
          getLocale() as UiLocale,
          surahAyahPathFor(resumeCtx, surah, targetPage.localPage, lastRead.n),
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
