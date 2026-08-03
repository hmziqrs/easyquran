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
  openerText: string | null;
}

export interface QuranSurahText {
  sourceId: QuranSourceId;
  script: QuranScript;
  verses: string[];
  normalization: SurahNormalization;
}

export type Place = "meccan" | "medinan";

export const Bismillah = {
  FirstAyah: "first-ayah",
  None: "none",
  EmbeddedPrefix: "embedded-prefix",
} as const;
export type Bismillah = (typeof Bismillah)[keyof typeof Bismillah];

export type SajdaKind = "recommended" | "obligatory";

export type VerseKey = string;

export interface CatalogEntry {
  num: number;
  slug: string;
  name: string;
  arabic: string;
  transliteration: string;
  meaning: string;
  place: Place;
  ayahCount: number;
  revelationOrder: number;
  rukus: number;
  openerKind: OpenerKind;
  bismillah: Bismillah;
  startGlobal: number;
}

export interface CanonicalSurahCoordinates {
  readonly surah: number;
  readonly startGlobal: number;
  readonly ayahCount: number;
}

export interface CanonicalQuranCoordinates {
  readonly rowCount: number;
  readonly surahs: readonly CanonicalSurahCoordinates[];
}

export type SurahRenderMetadata = Pick<
  CatalogEntry,
  "num" | "slug" | "name" | "arabic" | "place" | "ayahCount"
>;

export type SurahLink = Pick<CatalogEntry, "num" | "slug" | "name" | "arabic">;

export interface LoadedSurah extends SurahRenderMetadata, QuranSurahText {}

export interface RangeEntry {
  index: number;
  startGlobal: number;
  endGlobal: number;
  first: VerseKey;
  last: VerseKey;
}

export interface SajdaEntry {
  index: number;
  surah: number;
  ayah: number;
  globalIndex: number;
  kind: SajdaKind;
}

export interface ArtifactSpec {
  id: QuranSourceId;
  sizeBytes: number;
  sha256: string;
  downloadUrl: string;
}

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

export interface QuranRangeText {
  ayahs: Ayah[];
  normalizations: SurahNormalization[];
}

export interface SurahLocalPage {
  surah: number;
  localPage: number;
  globalPage: number;
  startGlobal: number;
  endGlobal: number;
  startAyah: number;
  endAyah: number;
  first: VerseKey;
  last: VerseKey;
}

export interface SurahLocalPageData {
  surah: SurahRenderMetadata;
  page: SurahLocalPage;
  pageCount: number;
  ayahs: Ayah[];
  normalization: SurahNormalization;
}

export interface SurahLocalPageLink {
  localPage: number;
  href: `/app/${string}`;
}

export interface SurahRouteData {
  pageData: SurahLocalPageData;
  previousPage: SurahLocalPageLink | null;
  nextPage: SurahLocalPageLink | null;
  previousSurah: SurahLink | null;
  nextSurah: SurahLink | null;
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
  surahs: SurahLink[];
}
