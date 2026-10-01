import { existsSync } from "node:fs";
import path from "node:path";
import { DatabaseSync } from "node:sqlite";

import { parseKey, surahMeta } from "$lib/data/quran";
import { RangeKind } from "$lib/data/quran-data";
import type { RangeEntry } from "$lib/data/quran-types";
import { TRANSLATION_BY_ID } from "$lib/data/translations";
import { displayVerses, headerText } from "$lib/quran/view/presentation";
import { bodyText } from "$lib/quran/view/source-view";
import { QURAN_DATA } from "$lib/server/quran-data";
import { readRangeText, readSurahText } from "$lib/server/quran-sqlite";

import type { MixData, MixRangeRow, MixSurahRow, MixTranslation } from "./types";

export const MIX_SURAH = 67;
const TRANSLATION_DIR = path.join("db", "quran", "translations");
const OPENING_WORDS = 3;
// Any Arabic-script letter; waqf marks and other standalone signs are not words.
const ARABIC_LETTER = /[ؠ-يٮ-ۓۺ-ۿ]/;

function translationFile(filePath: string): string | null {
  const candidates = [
    path.resolve(process.cwd(), "..", TRANSLATION_DIR, filePath),
    path.resolve(process.cwd(), TRANSLATION_DIR, filePath),
  ];
  return candidates.find((candidate) => existsSync(candidate)) ?? null;
}

/** Reads one surah of a Tanzil-schema translation sqlite; null when absent or unreadable. */
function readTranslation(id: string, surah: number, ayahCount: number): MixTranslation | null {
  const meta = TRANSLATION_BY_ID.get(id);
  if (!meta) return null;
  const file = translationFile(meta.filePath);
  if (!file) return null;
  const database = new DatabaseSync(file);
  try {
    database.exec("PRAGMA query_only = ON");
    const rows = database
      .prepare("SELECT aya, text FROM quran_text WHERE sura = ? ORDER BY aya")
      .all(surah);
    const texts = Array.from({ length: ayahCount }, () => "");
    for (const row of rows) {
      const aya = Number(row.aya);
      if (!Number.isInteger(aya) || aya < 1 || aya > ayahCount) continue;
      texts[aya - 1] = String(row.text ?? "");
    }
    return {
      id,
      name: meta.name,
      translator: meta.translator ?? meta.name,
      language: meta.language,
      languageCode: meta.languageCode,
      direction: meta.direction,
      texts,
    };
  } catch {
    return null;
  } finally {
    database.close();
  }
}

let openingCache: Map<number, string> | null = null;

/** First words of every ayah, keyed by global index (opener stripped from ayah 1). */
function openings(): Map<number, string> {
  if (openingCache) return openingCache;
  const range = readRangeText(1, QURAN_DATA.coordinates.rowCount);
  const normalizations = new Map(range.normalizations.map((n) => [n.surah, n]));
  const out = new Map<number, string>();
  for (const ayah of range.ayahs) {
    const normalization = normalizations.get(ayah.surah);
    const body = normalization ? bodyText(ayah.text, ayah.ayah, normalization) : ayah.text;
    const words = body
      .split(/\s+/)
      .filter((word) => ARABIC_LETTER.test(word))
      .slice(0, OPENING_WORDS);
    out.set(ayah.globalIndex, words.join(" "));
  }
  openingCache = out;
  return out;
}

function surahName(num: number): string {
  return QURAN_DATA.surahByNum(num)?.name ?? `Surah ${num}`;
}

function containing(ranges: readonly RangeEntry[], global: number): number {
  return ranges.find((r) => global >= r.startGlobal && global <= r.endGlobal)?.index ?? 1;
}

export function loadMixData(translationIds: readonly string[]): MixData {
  const cat = QURAN_DATA.surahByNum(MIX_SURAH)!;
  const text = readSurahText(cat.num);
  const juzRanges = QURAN_DATA.ranges(RangeKind.Juz);
  const pageRanges = QURAN_DATA.ranges(RangeKind.Page);
  const firstWords = openings();

  const translations: MixTranslation[] = [];
  const missing: string[] = [];
  for (const id of translationIds) {
    const t = readTranslation(id, cat.num, cat.ayahCount);
    if (t) translations.push(t);
    else missing.push(id);
  }

  const surahs: MixSurahRow[] = QURAN_DATA.surahs.map((s) => ({
    num: s.num,
    name: s.name,
    arabic: s.arabic,
    meaning: s.meaning,
    meta: surahMeta(s),
  }));

  const rangeRow = (r: RangeEntry, juz: number, pages: number): MixRangeRow => ({
    index: r.index,
    first: r.first,
    last: r.last,
    startName: surahName(parseKey(r.first).num),
    endName: surahName(parseKey(r.last).num),
    opening: firstWords.get(r.startGlobal) ?? "",
    juz,
    pages,
  });

  const juz = juzRanges.map((r) =>
    rangeRow(
      r,
      r.index,
      pageRanges.filter((p) => p.startGlobal >= r.startGlobal && p.startGlobal <= r.endGlobal)
        .length,
    ),
  );
  const pages = pageRanges.map((r) => rangeRow(r, containing(juzRanges, r.startGlobal), 1));

  return {
    surah: {
      num: cat.num,
      name: cat.name,
      arabic: cat.arabic,
      meaning: cat.meaning,
      meta: surahMeta(cat),
      pageCount: QURAN_DATA.surahLocalPageCount(cat.num),
      // SAFETY: ((num-1) % 4) is 0–3 for any positive integer, so +1 is exactly 1–4.
      hue: (((cat.num - 1) % 4) + 1) as 1 | 2 | 3 | 4,
    },
    opener: headerText(text.normalization),
    verses: displayVerses(text),
    translations,
    missing,
    surahs,
    juz,
    pages,
    activeJuz: containing(juzRanges, cat.startGlobal),
    activePage: containing(pageRanges, cat.startGlobal),
  };
}
