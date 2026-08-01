/* ════════════════════════════════════════════════════════════════════════
   quran-types.ts — the shared Quran content types.

   Kept free of imports so the build-time virtual module's ambient declaration
   (quran-meta.d.ts) can reference it without pulling in runes/store code. These
   mirror the wire contract in docs/quran-api.md §6.3 (camelCase) so the future
   live API and the offline Worker speak the same shape.
   ════════════════════════════════════════════════════════════════════════ */

/**
 * Runtime-safe enum pattern: unlike TypeScript `enum`, these values serialize
 * directly across JSON/postMessage and emit no reverse-mapping code.
 */
export const QuranSourceId = {
  TanzilUthmani: "uthmani",
  TanzilSimpleClean: "simple-clean",
} as const;
export type QuranSourceId = (typeof QuranSourceId)[keyof typeof QuranSourceId];
export const QURAN_SOURCE_IDS = Object.freeze(
  Object.values(QuranSourceId),
) as readonly QuranSourceId[];
export const isQuranSourceId = (value: unknown): value is QuranSourceId =>
  typeof value === "string" && QURAN_SOURCE_IDS.includes(value as QuranSourceId);

/** Text/script style is independent from the artifact/source that supplies it. */
export const QuranScript = {
  Uthmani: "uthmani",
  SimpleClean: "simple-clean",
  IndoPak: "indopak",
  Tajweed: "tajweed",
} as const;
export type QuranScript = (typeof QuranScript)[keyof typeof QuranScript];
export const QURAN_SCRIPTS = Object.freeze(Object.values(QuranScript)) as readonly QuranScript[];
export const isQuranScript = (value: unknown): value is QuranScript =>
  typeof value === "string" && QURAN_SCRIPTS.includes(value as QuranScript);

/** Source-independent opener meaning for the supported 6,236-ayah numbering. */
export const OpenerKind = {
  Verse: "verse",
  Header: "header",
  None: "none",
} as const;
export type OpenerKind = (typeof OpenerKind)[keyof typeof OpenerKind];
export const OPENER_KINDS = Object.freeze(Object.values(OpenerKind)) as readonly OpenerKind[];
export const isOpenerKind = (value: unknown): value is OpenerKind =>
  typeof value === "string" && OPENER_KINDS.includes(value as OpenerKind);

export const OpenerPackaging = {
  NumberedAyah: "numbered-ayah",
  EmbeddedPrefix: "embedded-prefix",
  ChapterFlag: "chapter-flag",
  SeparateRow: "separate-row",
  Absent: "absent",
} as const;
export type OpenerPackaging = (typeof OpenerPackaging)[keyof typeof OpenerPackaging];
export const OPENER_PACKAGING_VALUES = Object.freeze(
  Object.values(OpenerPackaging),
) as readonly OpenerPackaging[];
export const isOpenerPackaging = (value: unknown): value is OpenerPackaging =>
  typeof value === "string" && OPENER_PACKAGING_VALUES.includes(value as OpenerPackaging);

/** Portable Unicode-scalar partition of an embedded opener and ayah body. */
export interface PrefixCut {
  openerEndScalar: number;
  bodyStartScalar: number;
}

export interface SurahNormalization extends PrefixCut {
  surah: number;
  sourceId: QuranSourceId;
  script: QuranScript;
  sourceProfile: string;
  packaging: OpenerPackaging;
  openerKind: OpenerKind;
  /** Exact source orthography. Null only when openerKind is "none". */
  openerText: string | null;
}

export interface QuranSurahText {
  sourceId: QuranSourceId;
  script: QuranScript;
  verses: string[];
  normalization: SurahNormalization;
}

export type Place = "meccan" | "medinan";

/**
 * How a surah carries its basmala (docs/quran-api.md §3.3):
 *  - "first-ayah"      ayah 1 IS the basmala (surah 1) — no header line
 *  - "none"            no basmala at all (surah 9) — no header line
 *  - "embedded-prefix" ayah 1 begins with basmala + " " (the other 112) — header
 */
export const Bismillah = {
  FirstAyah: "first-ayah",
  None: "none",
  EmbeddedPrefix: "embedded-prefix",
} as const;
export type Bismillah = (typeof Bismillah)[keyof typeof Bismillah];

export type SajdaKind = "recommended" | "obligatory";

export type VerseKey = string;

/** Catalog metadata for a surah — no verse text (doc §10 split). */
export interface CatalogEntry {
  /** surah number, 1..114 */
  num: number;
  /** URL slug, e.g. "al-baqarah" — drives /app/<slug> deep links (web-owned) */
  slug: string;
  name: string;
  arabic: string;
  place: Place;
  /** ayah count (ayas from quran-data.xml) */
  ayahCount: number;
  /** revelation order (order from quran-data.xml) */
  revelationOrder: number;
  rukus: number;
  openerKind: OpenerKind;
  /** @deprecated Use openerKind. This field describes the current Tanzil shape. */
  bismillah: Bismillah;
  /** global index of this surah's first ayah (1-based; = xml start + 1) */
  startGlobal: number;
}

/** A loaded surah: catalog metadata + the Uthmani verse text, in order. This is
 *  the component-facing value (the SurahReader prop) and keeps the synchronous
 *  `verses` contract the reader store depends on. */
export interface LoadedSurah extends CatalogEntry, QuranSurahText {}

export interface RangeEntry {
  /** 1-based index within its family */
  index: number;
  /** inclusive first global ayah index */
  startGlobal: number;
  /** inclusive last global ayah index */
  endGlobal: number;
  first: VerseKey;
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

export interface SajdaEntry {
  index: number;
  surah: number;
  ayah: number;
  globalIndex: number;
  kind: SajdaKind;
}

/** A validated SQLite artifact advertised by /scripts or baked in config. */
export interface ArtifactSpec {
  id: QuranSourceId;
  sizeBytes: number;
  sha256: string;
  downloadUrl: string;
}

/** Live download progress for one Arabic artifact (drives a future progress bar).
 *  Named `script` (not `id`) so it never collides with the correlation `id` on
 *  WorkerResponse in the worker-client message router. */
export interface DownloadProgress {
  script: QuranSourceId;
  loaded: number;
  total: number;
}

export interface Ayah {
  key: VerseKey;
  surah: number;
  ayah: number;
  globalIndex: number;
  text: string;
}

export interface RangePageData {
  kind: "juz" | "page";
  index: number;
  label: string;
  startGlobal: number;
  endGlobal: number;
  first: VerseKey;
  last: VerseKey;
  ayahs: Ayah[];
  normalizations: SurahNormalization[];
}
