import {
  isArabicSourceId,
  type CatalogEntry,
  type LoadedSurah,
  type SurahRouteContext,
  type VerseKey,
} from "$lib/data/quran-types";

export type {
  CatalogEntry,
  LoadedSurah,
  VerseKey,
  Place,
  PrefixCut,
  SurahNormalization,
  SurahLink,
  SurahRouteContext,
  SurahRenderMetadata,
  QuranSurahText,
  SajdaKind,
  RangeEntry,
  SajdaEntry,
  ArtifactSpec,
  QuranRangeText,
  MushafPageLink,
  SurahLocalPage,
  SurahLocalPageData,
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
export const parseKey = (key: VerseKey) => {
  const m = /^(\d+):(\d+)$/.exec(key);
  if (!m) return { num: 1, n: 1 };
  return { num: +m[1]!, n: +m[2]! };
};

export const surahPath = (surah: string | Pick<CatalogEntry, "slug">): `/${string}` => {
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- narrowing the TS union (string | Pick<CatalogEntry,"slug">); typeof is the correct runtime test for a primitive string, no parse seam
  const slug = typeof surah === "string" ? surah : surah.slug;
  return `/${slug}`;
};

export const surahAyahPath = (
  surah: Pick<CatalogEntry, "slug" | "num">,
  ayah: number,
): `/${string}` => `${surahPath(surah)}#ayah-${surah.num}-${ayah}`;

export const translationIdFromSegments = (lang: string, translator: string): string =>
  translator === "" ? lang : `${lang}.${translator}`;

export const translationSegmentsFromId = (id: string) => {
  const dot = id.indexOf(".");
  if (dot < 0) return { lang: id, translator: "" };
  return { lang: id.slice(0, dot), translator: id.slice(dot + 1) };
};

export const translationSurahPath = (
  slug: string,
  lang: string,
  translator: string,
): `/${string}` => `/${slug}/t/${lang}/${translator}`;

export const translationGlobalPagePath = (
  lang: string,
  translator: string,
  globalPage: number,
): `/${string}` => `/t/${lang}/${translator}/page/${globalPage}`;

export const translationJuzPath = (lang: string, translator: string, n: number): `/${string}` =>
  `/t/${lang}/${translator}/juz/${n}`;

export const translationHizbPath = (lang: string, translator: string, n: number): `/${string}` =>
  `/t/${lang}/${translator}/hizb/${n}`;

export const translationRubPath = (lang: string, translator: string, n: number): `/${string}` =>
  `/t/${lang}/${translator}/rub/${n}`;

export const surahRouteContext = (sourceId: string): SurahRouteContext => {
  if (isArabicSourceId(sourceId)) return { kind: "arabic" };
  const { lang, translator } = translationSegmentsFromId(sourceId);
  return { kind: "translation", lang, translator };
};

export const routeContextFromParams = (
  params: Record<string, string | undefined>,
): SurahRouteContext => {
  const lang = params.lang;
  const translator = params.translator;
  return lang && translator ? { kind: "translation", lang, translator } : { kind: "arabic" };
};

export const resumeCtxFor = (
  lastRead: { sourceId?: string } | null,
  currentCtx: SurahRouteContext,
): SurahRouteContext => (lastRead?.sourceId ? surahRouteContext(lastRead.sourceId) : currentCtx);

export const surahPathFor = (
  ctx: SurahRouteContext,
  surah: string | Pick<CatalogEntry, "slug">,
): `/${string}` => {
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- narrowing the TS union (string | Pick<CatalogEntry,"slug">); typeof is the correct runtime test for a primitive string, no parse seam
  const slug = typeof surah === "string" ? surah : surah.slug;
  return ctx.kind === "arabic"
    ? surahPath(slug)
    : translationSurahPath(slug, ctx.lang, ctx.translator);
};

/**
 * Deep link to one ayah of a surah: the bare surah URL plus the ayah anchor.
 * One URL per surah (D1) — position rides in the fragment/query, never the path.
 */
export const surahAyahPathFor = (
  ctx: SurahRouteContext,
  surah: Pick<CatalogEntry, "slug" | "num">,
  ayah: number,
): `/${string}` => `${surahPathFor(ctx, surah)}#ayah-${surah.num}-${ayah}`;

export const globalPagePathFor = (ctx: SurahRouteContext, globalPage: number): `/${string}` =>
  ctx.kind === "arabic"
    ? `/page/${globalPage}`
    : translationGlobalPagePath(ctx.lang, ctx.translator, globalPage);

export const juzPathFor = (ctx: SurahRouteContext, n: number): `/${string}` =>
  ctx.kind === "arabic" ? `/juz/${n}` : translationJuzPath(ctx.lang, ctx.translator, n);

export const hizbPathFor = (ctx: SurahRouteContext, n: number): `/${string}` =>
  ctx.kind === "arabic" ? `/hizb/${n}` : translationHizbPath(ctx.lang, ctx.translator, n);

export const rubPathFor = (ctx: SurahRouteContext, n: number): `/${string}` =>
  ctx.kind === "arabic" ? `/rub/${n}` : translationRubPath(ctx.lang, ctx.translator, n);

const ARABIC_DIGITS = "٠١٢٣٤٥٦٧٨٩";

export const toArabicDigits = (n: number | string): string =>
  String(n).replace(/[0-9]/g, (d) => ARABIC_DIGITS[+d] ?? d);

export const surahMeta = (s: Pick<CatalogEntry, "place" | "ayahCount">): string =>
  `${s.place === "meccan" ? "Meccan" : "Medinan"} · ${s.ayahCount} verses`;

export const tafsirFor = (key: VerseKey): string => {
  const { num } = parseKey(key);
  return `Sample commentary for Surah ${num}, ${key} — in the full app this slot carries a short, credited tafsir summary.`;
};
