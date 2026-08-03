import { canonicalOpenerKind } from "$lib/quran/view/canonical";
import {
  Bismillah,
  type CanonicalQuranCoordinates,
  type CatalogEntry,
  type Place,
  type RangeEntry,
  type SajdaEntry,
  type SurahLocalPage,
  type VerseKey,
} from "$lib/data/quran-types";

export const QuranDataRoot = {
  Source: 0,
  Surahs: 1,
  Page: 2,
  Juz: 3,
  Ruku: 4,
  HizbQuarter: 5,
  Manzil: 6,
  Sajdas: 7,
} as const;

export const SurahField = {
  Slug: 0,
  DisplayName: 1,
  ArabicName: 2,
  TanzilTransliteration: 3,
  EnglishMeaning: 4,
  PlaceCode: 5,
  AyahCount: 6,
  RevelationOrder: 7,
  RukuCount: 8,
} as const;

export const SajdaField = { Surah: 0, Ayah: 1, KindCode: 2 } as const;

export const RangeKind = {
  Page: "page",
  Juz: "juz",
  Ruku: "ruku",
  HizbQuarter: "hizbQuarter",
  Manzil: "manzil",
} as const;
export type RangeKind = (typeof RangeKind)[keyof typeof RangeKind];

type SurahRow = readonly [string, string, string, string, string, number, number, number, number];
type SajdaRow = readonly [number, number, number];

export type QuranDataSource = readonly [digest: string, sourceVersion: string, license: string];

interface QuranDataSnapshot {
  readonly [QuranDataRoot.Source]: QuranDataSource;
  readonly [QuranDataRoot.Surahs]: readonly SurahRow[];
  readonly [QuranDataRoot.Page]: readonly number[];
  readonly [QuranDataRoot.Juz]: readonly number[];
  readonly [QuranDataRoot.Ruku]: readonly number[];
  readonly [QuranDataRoot.HizbQuarter]: readonly number[];
  readonly [QuranDataRoot.Manzil]: readonly number[];
  readonly [QuranDataRoot.Sajdas]: readonly SajdaRow[];
}

export interface QuranData {
  readonly source: QuranDataSource;
  readonly surahs: readonly CatalogEntry[];
  readonly coordinates: CanonicalQuranCoordinates;
  surahByNum(num: number): CatalogEntry | undefined;
  surahBySlug(slug: string): CatalogEntry | undefined;
  globalIndexOf(surah: number, ayah: number): number | undefined;
  verseKeyAtGlobal(globalIndex: number): VerseKey | undefined;
  rangeCount(kind: RangeKind): number;
  rangeByIndex(kind: RangeKind, index: number): RangeEntry | undefined;
  ranges(kind: RangeKind): readonly RangeEntry[];
  surahLocalPageCount(surah: number): number;
  surahLocalPage(surah: number, localPage: number): SurahLocalPage | undefined;
  surahLocalPages(surah: number): readonly SurahLocalPage[];
  surahLocalPageForAyah(surah: number, ayah: number): SurahLocalPage | undefined;
  sajdas(): readonly SajdaEntry[];
  sajdaAt(surah: number, ayah: number): SajdaEntry | undefined;
}

const EXPECTED_SURAHS = 114;
const EXPECTED_AYAHS = 6236;
const EXPECTED_RANGE_COUNTS: Readonly<Record<RangeKind, number>> = Object.freeze({
  [RangeKind.Page]: 604,
  [RangeKind.Juz]: 30,
  [RangeKind.Ruku]: 556,
  [RangeKind.HizbQuarter]: 240,
  [RangeKind.Manzil]: 7,
});

function fail(message: string): never {
  throw new Error(`[quran-data] ${message}`);
}

function integer(value: unknown, label: string, min = 0): number {
  if (!Number.isSafeInteger(value) || (value as number) < min) {
    fail(`${label} must be an integer >= ${min}`);
  }
  return value as number;
}

function nonemptyString(value: unknown, label: string): string {
  if (typeof value !== "string" || value.length === 0) fail(`${label} must be a non-empty string`);
  return value;
}

function sourceOf(raw: unknown): QuranDataSource {
  if (!Array.isArray(raw) || raw.length !== 3) fail("source must have three fields");
  const source = [
    nonemptyString(raw[0], "source digest"),
    nonemptyString(raw[1], "source version"),
    nonemptyString(raw[2], "source license"),
  ] as const;
  if (!/^[a-f0-9]{64}$/.test(source[0])) fail("source digest must be a lowercase SHA-256");
  return Object.freeze(source);
}

function placeOf(code: number): Place {
  if (code === 0) return "meccan";
  if (code === 1) return "medinan";
  return fail(`unknown place code ${code}`);
}

function bismillahOf(num: number): Bismillah {
  if (num === 1) return Bismillah.FirstAyah;
  if (num === 9) return Bismillah.None;
  return Bismillah.EmbeddedPrefix;
}

function decodeDeltas(deltas: readonly number[]): readonly number[] {
  if (deltas.length === 0 || deltas[0] !== 1) fail("range series must start at global ayah 1");
  const starts = [deltas[0]];
  for (let i = 1; i < deltas.length; i += 1) {
    const delta = integer(deltas[i], `range delta ${i}`, 1);
    starts.push(starts[i - 1]! + delta);
  }
  return Object.freeze(starts);
}

export function createQuranData(raw: unknown): QuranData {
  if (!Array.isArray(raw) || raw.length !== 8) fail("snapshot root must have eight fields");
  const snapshot = raw as unknown as QuranDataSnapshot;
  const source = sourceOf(snapshot[QuranDataRoot.Source]);
  const rows = snapshot[QuranDataRoot.Surahs];
  if (!Array.isArray(rows) || rows.length !== EXPECTED_SURAHS) {
    fail(`expected ${EXPECTED_SURAHS} Surah rows, got ${Array.isArray(rows) ? rows.length : 0}`);
  }

  let startGlobal = 1;
  const surahs = rows.map((rawRow, i) => {
    if (!Array.isArray(rawRow) || rawRow.length !== 9) fail(`invalid row for Surah ${i + 1}`);
    const row = rawRow as unknown as SurahRow;
    const num = i + 1;
    const ayahCount = integer(row[SurahField.AyahCount], `Surah ${num} ayah count`, 1);
    const entry: CatalogEntry = Object.freeze({
      num,
      slug: nonemptyString(row[SurahField.Slug], `Surah ${num} slug`),
      name: nonemptyString(row[SurahField.DisplayName], `Surah ${num} display name`),
      arabic: nonemptyString(row[SurahField.ArabicName], `Surah ${num} Arabic name`),
      transliteration: nonemptyString(
        row[SurahField.TanzilTransliteration],
        `Surah ${num} transliteration`,
      ),
      meaning: nonemptyString(row[SurahField.EnglishMeaning], `Surah ${num} meaning`),
      place: placeOf(integer(row[SurahField.PlaceCode], `Surah ${num} place code`)),
      ayahCount,
      revelationOrder: integer(row[SurahField.RevelationOrder], `Surah ${num} revelation order`, 1),
      rukus: integer(row[SurahField.RukuCount], `Surah ${num} ruku count`, 1),
      openerKind: canonicalOpenerKind(num),
      bismillah: bismillahOf(num),
      startGlobal,
    });
    startGlobal += ayahCount;
    return entry;
  });

  const rowCount = startGlobal - 1;
  if (rowCount !== EXPECTED_AYAHS) fail(`expected ${EXPECTED_AYAHS} ayahs, got ${rowCount}`);
  if (new Set(surahs.map((surah) => surah.slug)).size !== surahs.length) {
    fail("Surah slugs must be unique");
  }

  const frozenSurahs = Object.freeze(surahs);
  const bySlug = new Map(frozenSurahs.map((surah) => [surah.slug, surah]));
  const coordinates: CanonicalQuranCoordinates = Object.freeze({
    rowCount,
    surahs: Object.freeze(
      frozenSurahs.map((surah) =>
        Object.freeze({
          surah: surah.num,
          startGlobal: surah.startGlobal,
          ayahCount: surah.ayahCount,
        }),
      ),
    ),
  });

  const rootByKind: Readonly<Record<RangeKind, number>> = Object.freeze({
    [RangeKind.Page]: QuranDataRoot.Page,
    [RangeKind.Juz]: QuranDataRoot.Juz,
    [RangeKind.Ruku]: QuranDataRoot.Ruku,
    [RangeKind.HizbQuarter]: QuranDataRoot.HizbQuarter,
    [RangeKind.Manzil]: QuranDataRoot.Manzil,
  });
  const starts = new Map<RangeKind, readonly number[]>();
  for (const kind of Object.values(RangeKind)) {
    const rawSeries = snapshot[rootByKind[kind] as keyof QuranDataSnapshot];
    if (!Array.isArray(rawSeries) || rawSeries.length !== EXPECTED_RANGE_COUNTS[kind]) {
      fail(`expected ${EXPECTED_RANGE_COUNTS[kind]} ${kind} ranges`);
    }
    const decoded = decodeDeltas(rawSeries as readonly number[]);
    if (decoded.at(-1)! > coordinates.rowCount) fail(`${kind} starts past the Quran`);
    starts.set(kind, decoded);
  }

  const rangeCache = new Map<RangeKind, readonly RangeEntry[]>();
  const localPageCache = new Map<number, readonly SurahLocalPage[]>();
  let sajdaCache: readonly SajdaEntry[] | undefined;
  const data: QuranData = {
    source,
    surahs: frozenSurahs,
    coordinates,
    surahByNum(num) {
      return Number.isSafeInteger(num) ? frozenSurahs[num - 1] : undefined;
    },
    surahBySlug(slug) {
      return bySlug.get(slug);
    },
    globalIndexOf(surah, ayah) {
      const entry = data.surahByNum(surah);
      if (!entry || !Number.isSafeInteger(ayah) || ayah < 1 || ayah > entry.ayahCount) {
        return undefined;
      }
      return entry.startGlobal + ayah - 1;
    },
    verseKeyAtGlobal(globalIndex) {
      if (!Number.isSafeInteger(globalIndex) || globalIndex < 1 || globalIndex > rowCount) {
        return undefined;
      }
      let lo = 0;
      let hi = frozenSurahs.length - 1;
      while (lo <= hi) {
        const mid = (lo + hi) >> 1;
        const surah = frozenSurahs[mid]!;
        const endGlobal = surah.startGlobal + surah.ayahCount - 1;
        if (globalIndex < surah.startGlobal) hi = mid - 1;
        else if (globalIndex > endGlobal) lo = mid + 1;
        else return `${surah.num}:${globalIndex - surah.startGlobal + 1}`;
      }
      return undefined;
    },
    rangeCount(kind) {
      return starts.get(kind)?.length ?? 0;
    },
    rangeByIndex(kind, index) {
      const family = starts.get(kind);
      if (!family || !Number.isSafeInteger(index) || index < 1 || index > family.length) {
        return undefined;
      }
      const start = family[index - 1]!;
      const end = index < family.length ? family[index]! - 1 : coordinates.rowCount;
      const first = data.verseKeyAtGlobal(start);
      const last = data.verseKeyAtGlobal(end);
      if (!first || !last) fail(`${kind} ${index} is outside canonical coordinates`);
      return Object.freeze({ index, startGlobal: start, endGlobal: end, first, last });
    },
    ranges(kind) {
      const cached = rangeCache.get(kind);
      if (cached) return cached;
      const family = Object.freeze(
        Array.from(
          { length: data.rangeCount(kind) },
          (_, index) => data.rangeByIndex(kind, index + 1)!,
        ),
      );
      rangeCache.set(kind, family);
      return family;
    },
    surahLocalPageCount(surah) {
      return data.surahLocalPages(surah).length;
    },
    surahLocalPage(surah, localPage) {
      if (!Number.isSafeInteger(localPage) || localPage < 1) return undefined;
      return data.surahLocalPages(surah)[localPage - 1];
    },
    surahLocalPages(surah) {
      const cached = localPageCache.get(surah);
      if (cached) return cached;
      const entry = data.surahByNum(surah);
      if (!entry) return Object.freeze([]);
      const surahEnd = entry.startGlobal + entry.ayahCount - 1;
      const pages = data
        .ranges(RangeKind.Page)
        .filter((page) => page.endGlobal >= entry.startGlobal && page.startGlobal <= surahEnd)
        .map((page, index) => {
          const startGlobal = Math.max(page.startGlobal, entry.startGlobal);
          const endGlobal = Math.min(page.endGlobal, surahEnd);
          return Object.freeze({
            surah,
            localPage: index + 1,
            globalPage: page.index,
            startGlobal,
            endGlobal,
            startAyah: startGlobal - entry.startGlobal + 1,
            endAyah: endGlobal - entry.startGlobal + 1,
            first: `${surah}:${startGlobal - entry.startGlobal + 1}`,
            last: `${surah}:${endGlobal - entry.startGlobal + 1}`,
          });
        });
      const frozen = Object.freeze(pages);
      localPageCache.set(surah, frozen);
      return frozen;
    },
    surahLocalPageForAyah(surah, ayah) {
      const globalIndex = data.globalIndexOf(surah, ayah);
      if (!globalIndex) return undefined;
      return data
        .surahLocalPages(surah)
        .find((page) => globalIndex >= page.startGlobal && globalIndex <= page.endGlobal);
    },
    sajdas() {
      if (sajdaCache) return sajdaCache;
      const sajdaRows = snapshot[QuranDataRoot.Sajdas];
      if (!Array.isArray(sajdaRows) || sajdaRows.length !== 15) fail("expected 15 sajda markers");
      sajdaCache = Object.freeze(
        sajdaRows.map((row, index) => {
          if (!Array.isArray(row) || row.length !== 3) fail(`invalid sajda row ${index + 1}`);
          const surah = row[SajdaField.Surah];
          const ayah = row[SajdaField.Ayah];
          const kindCode = row[SajdaField.KindCode];
          const globalIndex = data.globalIndexOf(surah, ayah);
          if (!globalIndex || (kindCode !== 0 && kindCode !== 1)) {
            fail(`invalid sajda row ${index + 1}`);
          }
          return Object.freeze({
            index: index + 1,
            surah,
            ayah,
            globalIndex,
            kind: kindCode === 1 ? "obligatory" : "recommended",
          });
        }),
      );
      return sajdaCache;
    },
    sajdaAt(surah, ayah) {
      return data.sajdas().find((entry) => entry.surah === surah && entry.ayah === ayah);
    },
  };
  return Object.freeze(data);
}
