import { JUZ_ALIASES, PAGE_ALIASES, SURAH_ALIASES } from "../aliases";
import { PaletteGroups } from "../groups";
import { hasKeyword, isBareNumber, referenceNumbers } from "../query";
import {
  JUZ_COUNT,
  MUSHAF_PAGE_COUNT,
  SURAH_COUNT,
  ayahHref,
  juzHref,
  openVerse,
  pageHref,
  surahDetail,
  surahHref,
} from "../quran-nav";
import type { PaletteEntry, PaletteQuery, PaletteSource } from "../types";

const inRange = (n: number, max: number): boolean => Number.isInteger(n) && n >= 1 && n <= max;

function surahEntry(query: PaletteQuery, num: number, score: number): PaletteEntry | null {
  const surah = query.quranData.surahByNum(num);
  const href = surahHref(query.routeContext, query.quranData, num);
  if (!surah || !href) return null;
  return {
    id: `quran.reference:surah:${num}`,
    sourceId: quranReferenceSource.id,
    groupId: PaletteGroups.JumpTo.id,
    label: `${surah.num}. ${surah.name}`,
    detail: surahDetail(surah),
    arabic: surah.arabic,
    icon: "book",
    score,
    href,
    dedupeKey: `surah:${num}`,
  };
}

function ayahEntry(query: PaletteQuery, num: number, ayah: number): PaletteEntry | null {
  const surah = query.quranData.surahByNum(num);
  if (!surah || ayah > surah.ayahCount) return null;
  const href = ayahHref(query.routeContext, query.quranData, num, ayah);
  if (!href) return null;
  return {
    id: `quran.reference:ayah:${num}:${ayah}`,
    sourceId: quranReferenceSource.id,
    groupId: PaletteGroups.JumpTo.id,
    label: `${surah.name} ${num}:${ayah}`,
    detail: "Verse",
    arabic: surah.arabic,
    icon: "book",
    score: 1,
    href,
    run: openVerse(num, ayah),
    dedupeKey: `ayah:${num}:${ayah}`,
  };
}

const juzEntry = (query: PaletteQuery, n: number, score: number): PaletteEntry => ({
  id: `quran.reference:juz:${n}`,
  sourceId: quranReferenceSource.id,
  groupId: PaletteGroups.JumpTo.id,
  label: `Juz ${n}`,
  detail: "Juz",
  icon: "rows",
  score,
  href: juzHref(query.routeContext, n),
  dedupeKey: `juz:${n}`,
});

const pageEntry = (query: PaletteQuery, n: number, score: number): PaletteEntry => ({
  id: `quran.reference:page:${n}`,
  sourceId: quranReferenceSource.id,
  groupId: PaletteGroups.JumpTo.id,
  label: `Page ${n}`,
  detail: "Mushaf page",
  icon: "rows",
  score,
  href: pageHref(query.routeContext, n),
  dedupeKey: `page:${n}`,
});

/**
 * The shorthand people actually type: `2:255`, `2 255`, `surah 18`, `juz 5`,
 * `p 100`, or a bare number — which stays ambiguous on purpose and offers the
 * surah, the juz and the page at once.
 */
export const quranReferenceSource: PaletteSource = {
  id: "quran.reference",
  groups: [PaletteGroups.JumpTo],
  limit: 4,

  enabled: ({ parsed }) => parsed.numbers.length > 0,

  entries(query) {
    const { parsed } = query;
    const ref = referenceNumbers(parsed);
    if (!ref) return [];
    const entries: (PaletteEntry | null)[] = [];

    if (hasKeyword(parsed, SURAH_ALIASES)) {
      if (!inRange(ref.primary, SURAH_COUNT)) return [];
      entries.push(
        ref.secondary === undefined
          ? surahEntry(query, ref.primary, 1)
          : ayahEntry(query, ref.primary, ref.secondary),
      );
    } else if (hasKeyword(parsed, JUZ_ALIASES)) {
      if (inRange(ref.primary, JUZ_COUNT)) entries.push(juzEntry(query, ref.primary, 1));
    } else if (hasKeyword(parsed, PAGE_ALIASES)) {
      if (inRange(ref.primary, MUSHAF_PAGE_COUNT)) entries.push(pageEntry(query, ref.primary, 1));
    } else if (parsed.keyword === null) {
      if (ref.secondary !== undefined) {
        if (inRange(ref.primary, SURAH_COUNT))
          entries.push(ayahEntry(query, ref.primary, ref.secondary));
      } else if (isBareNumber(parsed)) {
        if (inRange(ref.primary, SURAH_COUNT)) entries.push(surahEntry(query, ref.primary, 1));
        if (inRange(ref.primary, JUZ_COUNT)) entries.push(juzEntry(query, ref.primary, 0.9));
        if (inRange(ref.primary, MUSHAF_PAGE_COUNT))
          entries.push(pageEntry(query, ref.primary, 0.8));
      }
    }

    return entries.filter((entry): entry is PaletteEntry => entry !== null);
  },
};
