import { readFileSync } from "node:fs";
import path from "node:path";
import { REPO } from "./config.ts";

const META = path.join(REPO, "web/static/quran-meta/quran-data.json");
const CATALOGUE = path.join(REPO, "web/src/lib/data/translations.json");

/** Realism ranking for translation popularity. Everything unlisted keeps catalogue order behind these. */
const POPULAR_TRANSLATIONS = [
  "en.sahih",
  "en.pickthall",
  "en.yusufali",
  "ur.jalandhry",
  "id.indonesian",
  "tr.diyanet",
  "fr.hamidullah",
  "bn.bengali",
  "ru.kuliev",
  "es.cortes",
];

/** Surahs people actually open. Rank order feeds the index Zipf. */
const POPULAR_SURAHS = [1, 2, 18, 36, 55, 67, 112, 113, 114, 3, 4, 19, 56, 78];

export interface Surah {
  readonly num: number;
  readonly slug: string;
  readonly localPages: number;
}

export interface Keyspace {
  /** Translation ids, most popular first. */
  readonly translations: readonly string[];
  /** Surahs, most popular first. */
  readonly surahs: readonly Surah[];
}

function decodeDeltas(deltas: readonly number[]): number[] {
  const starts = [deltas[0]!];
  for (let i = 1; i < deltas.length; i += 1) starts.push(starts[i - 1]! + deltas[i]!);
  return starts;
}

export function loadKeyspace(): Keyspace {
  const meta = JSON.parse(readFileSync(META, "utf8")) as unknown[];
  const catalogue = JSON.parse(readFileSync(CATALOGUE, "utf8")) as unknown[][];

  const surahRows = meta[1] as unknown[][];
  const pageStarts = decodeDeltas(meta[2] as number[]);
  const rowCount = 6236;

  // Page ranges as [startGlobal, endGlobal], the tiling the web reader clips against.
  const pages = pageStarts.map((start, i) => ({
    start,
    end: i + 1 < pageStarts.length ? pageStarts[i + 1]! - 1 : rowCount,
  }));

  let startGlobal = 1;
  const surahs: Surah[] = surahRows.map((row, i) => {
    const ayahCount = row[6] as number;
    const first = startGlobal;
    const last = startGlobal + ayahCount - 1;
    startGlobal += ayahCount;
    return {
      num: i + 1,
      slug: row[0] as string,
      localPages: pages.filter((page) => page.end >= first && page.start <= last).length,
    };
  });
  if (surahs.length !== 114) throw new Error(`[keyspace] expected 114 surahs, got ${surahs.length}`);
  const totalLocalPages = surahs.reduce((sum, s) => sum + s.localPages, 0);
  if (totalLocalPages !== 662) {
    throw new Error(`[keyspace] expected 662 surah-local pages, got ${totalLocalPages}`);
  }

  const ids = catalogue.map((row) => row[0] as string);
  const rank = (id: string): number => {
    const i = POPULAR_TRANSLATIONS.indexOf(id);
    return i === -1 ? POPULAR_TRANSLATIONS.length + ids.indexOf(id) : i;
  };
  const translations = [...ids].sort((a, b) => rank(a) - rank(b));

  const surahRank = (num: number): number => {
    const i = POPULAR_SURAHS.indexOf(num);
    return i === -1 ? POPULAR_SURAHS.length + num : i;
  };
  const ranked = [...surahs].sort((a, b) => surahRank(a.num) - surahRank(b.num));

  return { translations, surahs: ranked };
}

/** `en.sahih` -> `/t/en/sahih`, `ber.mensur` -> `/t/ber/mensur`. Mirrors translationIdFromSegments. */
export function segmentsOf(id: string): { lang: string; translator: string } {
  const dot = id.indexOf(".");
  return dot < 0
    ? { lang: id, translator: "" }
    : { lang: id.slice(0, dot), translator: id.slice(dot + 1) };
}
