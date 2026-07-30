/* ════════════════════════════════════════════════════════════════════════
   quran-types.ts — the shared Quran content types.

   Kept free of imports so the build-time virtual module's ambient declaration
   (quran-meta.d.ts) can reference it without pulling in runes/store code. These
   mirror the wire contract in docs/quran-api.md §6.3 (camelCase) so the future
   live API and the offline Worker speak the same shape.
   ════════════════════════════════════════════════════════════════════════ */

/** The two Arabic sources the reader works against. */
export type Script = "uthmani" | "simple-clean";

/** Revelation place. */
export type Place = "meccan" | "medinan";

/**
 * How a surah carries its basmala (docs/quran-api.md §3.3):
 *  - "first-ayah"      ayah 1 IS the basmala (surah 1) — no header line
 *  - "none"            no basmala at all (surah 9) — no header line
 *  - "embedded-prefix" ayah 1 begins with basmala + " " (the other 112) — header
 */
export type Bismillah = "first-ayah" | "none" | "embedded-prefix";

/** Sajda kind on a verse. */
export type SajdaKind = "recommended" | "obligatory";

/** A coordinate into the text — "surah:ayah". */
export type VerseKey = string;

/** Catalog metadata for a surah — no verse text (doc §10 split). */
export interface CatalogEntry {
  /** surah number, 1..114 */
  num: number;
  /** URL slug, e.g. "al-baqarah" — drives /app/<slug> deep links (web-owned) */
  slug: string;
  /** transliterated display name, e.g. "Al-Fatihah" */
  name: string;
  /** Arabic name, e.g. "الفاتحة" */
  arabic: string;
  place: Place;
  /** ayah count (ayas from quran-data.xml) */
  ayahCount: number;
  /** revelation order (order from quran-data.xml) */
  revelationOrder: number;
  /** ruku count for this surah */
  rukus: number;
  bismillah: Bismillah;
  /** global index of this surah's first ayah (1-based; = xml start + 1) */
  startGlobal: number;
}

/** A loaded surah: catalog metadata + the Uthmani verse text, in order. This is
 *  the component-facing value (the SurahReader prop) and keeps the synchronous
 *  `verses` contract the reader store depends on. */
export interface LoadedSurah extends CatalogEntry {
  verses: string[];
}

/** A navigation range family (juz / page / ruku / hizb-quarter / manzil). */
export interface RangeEntry {
  /** 1-based index within its family */
  index: number;
  /** inclusive first global ayah index */
  startGlobal: number;
  /** inclusive last global ayah index */
  endGlobal: number;
  /** surah:ayah of the first ayah */
  first: VerseKey;
  /** surah:ayah of the last ayah */
  last: VerseKey;
}

/** All navigation families, derived from quran-data.xml start markers. */
export interface NavigationData {
  juz: RangeEntry[];
  page: RangeEntry[];
  ruku: RangeEntry[];
  hizbQuarter: RangeEntry[];
  manzil: RangeEntry[];
}

/** A sajda marker (not a range). */
export interface SajdaEntry {
  index: number;
  surah: number;
  ayah: number;
  globalIndex: number;
  kind: SajdaKind;
}

/** A validated SQLite artifact advertised by /scripts or baked in config. */
export interface ArtifactSpec {
  id: Script;
  sizeBytes: number;
  sha256: string;
  downloadUrl: string;
}
