import type { CatalogEntry, LoadedSurah, VerseKey } from "$lib/data/quran-types";

export type {
  CatalogEntry,
  LoadedSurah,
  VerseKey,
  Place,
  PrefixCut,
  SurahNormalization,
  QuranSurahText,
  SajdaKind,
  RangeEntry,
  NavigationData,
  SajdaEntry,
  ArtifactSpec,
} from "$lib/data/quran-types";

export {
  Bismillah,
  OpenerKind,
  OpenerPackaging,
  QuranScript,
  QuranSourceId,
} from "$lib/data/quran-types";

export type Surah = LoadedSurah;

export const verseKey = (surah: number, ayah: number): VerseKey => `${surah}:${ayah}`;
export const parseKey = (key: VerseKey): { num: number; n: number } => {
  const m = /^(\d+):(\d+)$/.exec(key);
  if (!m) return { num: 1, n: 1 };
  return { num: +m[1]!, n: +m[2]! };
};

export const surahPath = (
  surah: string | Pick<CatalogEntry, "slug">,
  ayah?: number,
): `/app/${string}` => {
  const slug = typeof surah === "string" ? surah : surah.slug;
  return `/app/${slug}${ayah ? `?verse=${ayah}` : ""}`;
};

export const toArabicDigits = (n: number | string): string =>
  String(n).replace(/[0-9]/g, (d) => "٠١٢٣٤٥٦٧٨٩"[+d]);

export const surahMeta = (s: Pick<CatalogEntry, "place" | "ayahCount">): string =>
  `${s.place === "meccan" ? "Meccan" : "Medinan"} · ${s.ayahCount} verses`;

export const tafsirFor = (key: VerseKey): string => {
  const { num } = parseKey(key);
  return `Sample commentary for Surah ${num}, ${key} — in the full app this slot carries a short, credited tafsir summary.`;
};
