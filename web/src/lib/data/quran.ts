import type { CatalogEntry, LoadedSurah, VerseKey } from "$lib/data/quran-types";

export type {
  CatalogEntry,
  LoadedSurah,
  VerseKey,
  Place,
  PrefixCut,
  SurahNormalization,
  SurahLink,
  SurahRenderMetadata,
  QuranSurahText,
  SajdaKind,
  RangeEntry,
  SajdaEntry,
  ArtifactSpec,
  QuranRangeText,
  SurahLocalPage,
  SurahLocalPageData,
  SurahLocalPageLink,
  SurahRouteData,
} from "$lib/data/quran-types";

export {
  Bismillah,
  OpenerKind,
  OpenerPackaging,
  QuranScript,
  QuranSourceId,
  SourceKind,
} from "$lib/data/quran-types";

export type Surah = LoadedSurah;

export const verseKey = (surah: number, ayah: number): VerseKey => `${surah}:${ayah}`;
export const parseKey = (key: VerseKey): { num: number; n: number } => {
  const m = /^(\d+):(\d+)$/.exec(key);
  if (!m) return { num: 1, n: 1 };
  return { num: +m[1]!, n: +m[2]! };
};

export const surahPath = (surah: string | Pick<CatalogEntry, "slug">): `/app/${string}` => {
  const slug = typeof surah === "string" ? surah : surah.slug;
  return `/app/${slug}`;
};

export const surahLocalPagePath = (
  surah: string | Pick<CatalogEntry, "slug">,
  localPage: number,
): `/app/${string}` => {
  const slug = typeof surah === "string" ? surah : surah.slug;
  return localPage > 1 ? `/app/${slug}/page/${localPage}` : `/app/${slug}`;
};

export const surahAyahPath = (
  surah: Pick<CatalogEntry, "slug" | "num">,
  localPage: number,
  ayah: number,
): `/app/${string}` => `${surahLocalPagePath(surah, localPage)}#ayah-${surah.num}-${ayah}`;

export const translationIdFromSegments = (lang: string, translator: string): string =>
  translator === "" ? lang : `${lang}.${translator}`;

export const translationSegmentsFromId = (id: string): { lang: string; translator: string } => {
  const dot = id.indexOf(".");
  if (dot < 0) return { lang: id, translator: "" };
  return { lang: id.slice(0, dot), translator: id.slice(dot + 1) };
};

export const translationSurahPath = (
  slug: string,
  lang: string,
  translator: string,
  localPage = 1,
): `/app/${string}` =>
  localPage > 1
    ? `/app/${slug}/t/${lang}/${translator}/page/${localPage}`
    : `/app/${slug}/t/${lang}/${translator}`;

export const translationGlobalPagePath = (
  lang: string,
  translator: string,
  globalPage: number,
): `/app/${string}` => `/app/t/${lang}/${translator}/page/${globalPage}`;

export const translationJuzPath = (lang: string, translator: string, n: number): `/app/${string}` =>
  `/app/t/${lang}/${translator}/juz/${n}`;

export const toArabicDigits = (n: number | string): string =>
  String(n).replace(/[0-9]/g, (d) => "٠١٢٣٤٥٦٧٨٩"[+d]);

export const surahMeta = (s: Pick<CatalogEntry, "place" | "ayahCount">): string =>
  `${s.place === "meccan" ? "Meccan" : "Medinan"} · ${s.ayahCount} verses`;

export const tafsirFor = (key: VerseKey): string => {
  const { num } = parseKey(key);
  return `Sample commentary for Surah ${num}, ${key} — in the full app this slot carries a short, credited tafsir summary.`;
};
