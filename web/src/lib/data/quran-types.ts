export const QuranSourceId = {
  TanzilUthmani: "uthmani",
  TanzilSimpleClean: "simple-clean",
} as const;
export type QuranSourceId = (typeof QuranSourceId)[keyof typeof QuranSourceId];
// SAFETY: Object.values on the `as const` QuranSourceId map yields ("uthmani" | "simple-clean")[]; casting to its readonly form is a covariance-only widening, not a value change.
export const QURAN_SOURCE_IDS = Object.freeze(
  Object.values(QuranSourceId),
) as readonly QuranSourceId[];
export const isQuranSourceId = (
  // eslint-disable-next-line anti-slop/no-unknown-parameters -- exported type guard that validates untrusted URL/catalogue input at the boundary; unknown is the correct input contract for a parser
  value: unknown,
): value is QuranSourceId => {
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- type-guard body: typeof string is the only runtime primitive test for an unknown value
  // SAFETY: the QURAN_SOURCE_IDS.includes() membership check below verifies value is one of the QuranSourceId literals before it is asserted as one.
  return typeof value === "string" && QURAN_SOURCE_IDS.includes(value as QuranSourceId);
};

export type QuranReaderSource = string;

export const isArabicSourceId = isQuranSourceId;

export const QuranScript = {
  Uthmani: "uthmani",
  SimpleClean: "simple-clean",
  IndoPak: "indopak",
  Tajweed: "tajweed",
  Translation: "translation",
} as const;
export type QuranScript = (typeof QuranScript)[keyof typeof QuranScript];
// SAFETY: Object.values on the `as const` QuranScript map yields the literal-union array; casting to its readonly form is a covariance-only widening, not a value change.
export const QURAN_SCRIPTS = Object.freeze(Object.values(QuranScript)) as readonly QuranScript[];
export const isQuranScript = (
  // eslint-disable-next-line anti-slop/no-unknown-parameters -- exported type guard that validates untrusted URL/catalogue input at the boundary; unknown is the correct input contract for a parser
  value: unknown,
): value is QuranScript => {
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- type-guard body: typeof string is the only runtime primitive test for an unknown value
  // SAFETY: the QURAN_SCRIPTS.includes() membership check below verifies value is one of the QuranScript literals before it is asserted as one.
  return typeof value === "string" && QURAN_SCRIPTS.includes(value as QuranScript);
};

export const OpenerKind = {
  Verse: "verse",
  Header: "header",
  None: "none",
} as const;
export type OpenerKind = (typeof OpenerKind)[keyof typeof OpenerKind];
// SAFETY: Object.values on the `as const` OpenerKind map yields the literal-union array; casting to its readonly form is a covariance-only widening, not a value change.
export const OPENER_KINDS = Object.freeze(Object.values(OpenerKind)) as readonly OpenerKind[];
export const isOpenerKind = (
  // eslint-disable-next-line anti-slop/no-unknown-parameters -- exported type guard that validates untrusted catalogue input at the boundary; unknown is the correct input contract for a parser
  value: unknown,
): value is OpenerKind => {
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- type-guard body: typeof string is the only runtime primitive test for an unknown value
  // SAFETY: the OPENER_KINDS.includes() membership check below verifies value is one of the OpenerKind literals before it is asserted as one.
  return typeof value === "string" && OPENER_KINDS.includes(value as OpenerKind);
};

export const OpenerPackaging = {
  NumberedAyah: "numbered-ayah",
  EmbeddedPrefix: "embedded-prefix",
  ChapterFlag: "chapter-flag",
  SeparateRow: "separate-row",
  Absent: "absent",
} as const;
export type OpenerPackaging = (typeof OpenerPackaging)[keyof typeof OpenerPackaging];
// SAFETY: Object.values on the `as const` OpenerPackaging map yields the literal-union array; casting to its readonly form is a covariance-only widening, not a value change.
export const OPENER_PACKAGING_VALUES = Object.freeze(
  Object.values(OpenerPackaging),
) as readonly OpenerPackaging[];
export const isOpenerPackaging = (
  // eslint-disable-next-line anti-slop/no-unknown-parameters -- exported type guard that validates untrusted catalogue input at the boundary; unknown is the correct input contract for a parser
  value: unknown,
): value is OpenerPackaging => {
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- type-guard body: typeof string is the only runtime primitive test for an unknown value
  // SAFETY: the OPENER_PACKAGING_VALUES.includes() membership check below verifies value is one of the OpenerPackaging literals before it is asserted as one.
  return typeof value === "string" && OPENER_PACKAGING_VALUES.includes(value as OpenerPackaging);
};

export interface PrefixCut {
  openerEndScalar: number;
  bodyStartScalar: number;
}

export interface SurahNormalization extends PrefixCut {
  surah: number;
  sourceId: QuranReaderSource;
  script: QuranScript;
  sourceProfile: string;
  packaging: OpenerPackaging;
  openerKind: OpenerKind;
  openerText: string | null;
}

export interface QuranSurahText {
  sourceId: QuranReaderSource;
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

export interface DownloadableSpec {
  id: string;
  sizeBytes: number;
  downloadUrl: string;
}

export interface ArtifactSpec extends DownloadableSpec {
  id: QuranSourceId;
}

export type TranslationDirection = "rtl" | "ltr";

export interface TranslationCatalogueEntry {
  id: string;
  language: string;
  languageCode: string;
  direction: TranslationDirection;
  name: string;
  translator: string | null;
  sizeBytes: number;
  downloadUrl: string;
}

export const SourceKind = {
  Arabic: "arabic",
  Translation: "translation",
} as const;
export type SourceKind = (typeof SourceKind)[keyof typeof SourceKind];

export type SourceCatalogueEntry =
  | { kind: "arabic"; spec: ArtifactSpec }
  | { kind: "translation"; entry: TranslationCatalogueEntry };

export interface DownloadProgress {
  script: QuranReaderSource;
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

export type SurahRouteContext =
  | { readonly kind: "arabic" }
  | { readonly kind: "translation"; readonly lang: string; readonly translator: string };

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
