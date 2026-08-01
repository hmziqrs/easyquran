import { CATALOG } from "$lib/data/quran-meta";
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

export const SURAHS: CatalogEntry[] = CATALOG;

const byNum = new Map<number, CatalogEntry>(CATALOG.map((s) => [s.num, s]));
const bySlug = new Map<string, CatalogEntry>(CATALOG.map((s) => [s.slug, s]));

export const surahByNum = (num: number): CatalogEntry => byNum.get(num) ?? CATALOG[0]!;
export const surahBySlug = (slug: string): CatalogEntry => bySlug.get(slug) ?? CATALOG[0]!;
export const slugFor = (num: number): string => surahByNum(num).slug;

export const surahPath = (num: number, ayah?: number): `/app/${string}` =>
  `/app/${slugFor(num)}${ayah ? `?verse=${ayah}` : ""}`;

export const toArabicDigits = (n: number | string): string =>
  String(n).replace(/[0-9]/g, (d) => "٠١٢٣٤٥٦٧٨٩"[+d]);

export const adjacentSurahs = (num: number): { prev: CatalogEntry; next: CatalogEntry } => {
  const idx = CATALOG.findIndex((s) => s.num === surahByNum(num).num);
  const len = CATALOG.length;
  return {
    prev: CATALOG[(idx - 1 + len) % len]!,
    next: CATALOG[(idx + 1) % len]!,
  };
};

export interface SearchResult {
  key: VerseKey;
  ref: string;
  text: string;
  num: number;
  ayah: number;
}

export function searchVerses(rawQuery: string): SearchResult[] {
  const q = rawQuery.trim();
  if (!q) return [];
  const qLower = q.toLowerCase();
  const out: SearchResult[] = [];
  for (const s of CATALOG) {
    if (out.length >= 24) break;
    const hit =
      s.name.toLowerCase().includes(qLower) || s.arabic.includes(q) || String(s.num) === qLower;
    if (hit) {
      out.push({
        key: verseKey(s.num, 1),
        ref: `${s.name} ${s.num}`,
        text: "",
        num: s.num,
        ayah: 1,
      });
    }
  }
  return out;
}

export const surahMeta = (s: CatalogEntry): string =>
  `${s.place === "meccan" ? "Meccan" : "Medinan"} · ${s.ayahCount} verses`;

export const tafsirFor = (key: VerseKey): string => {
  const { num } = parseKey(key);
  const s = surahByNum(num);
  return `Sample commentary for ${s.name} ${key} — in the full app this slot carries a short, credited tafsir summary.`;
};
