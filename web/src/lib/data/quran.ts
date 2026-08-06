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
  SurahLocalPage,
  SurahLocalPageData,
  SurahLocalPageLink,
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
export const parseKey = (key: VerseKey): { num: number; n: number } => {
  const m = /^(\d+):(\d+)$/.exec(key);
  if (!m) return { num: 1, n: 1 };
  return { num: +m[1]!, n: +m[2]! };
};

export const surahPath = (surah: string | Pick<CatalogEntry, "slug">): `/app/${string}` => {
  const slug = typeof surah === "string" ? surah : surah.slug;
  return `/app/${slug}`;
};

export const surahLocalPagePath = (
  surah: string | Pick<CatalogEntry, "slug">,
  localPage: number,
): `/app/${string}` => {
  const slug = typeof surah === "string" ? surah : surah.slug;
  return localPage > 1 ? `/app/${slug}/page/${localPage}` : `/app/${slug}`;
};

export const surahAyahPath = (
  surah: Pick<CatalogEntry, "slug" | "num">,
  localPage: number,
  ayah: number,
): `/app/${string}` => `${surahLocalPagePath(surah, localPage)}#ayah-${surah.num}-${ayah}`;

export const translationIdFromSegments = (lang: string, translator: string): string =>
  translator === "" ? lang : `${lang}.${translator}`;

export const translationSegmentsFromId = (id: string): { lang: string; translator: string } => {
  const dot = id.indexOf(".");
  if (dot < 0) return { lang: id, translator: "" };
  return { lang: id.slice(0, dot), translator: id.slice(dot + 1) };
};

export const translationSurahPath = (
  slug: string,
  lang: string,
  translator: string,
  localPage = 1,
): `/app/${string}` =>
  localPage > 1
    ? `/app/${slug}/t/${lang}/${translator}/page/${localPage}`
    : `/app/${slug}/t/${lang}/${translator}`;

export const translationGlobalPagePath = (
  lang: string,
  translator: string,
  globalPage: number,
): `/app/${string}` => `/app/t/${lang}/${translator}/page/${globalPage}`;

export const translationJuzPath = (lang: string, translator: string, n: number): `/app/${string}` =>
  `/app/t/${lang}/${translator}/juz/${n}`;

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

export const surahPathFor = (
  ctx: SurahRouteContext,
  surah: string | Pick<CatalogEntry, "slug">,
): `/app/${string}` => {
  const slug = typeof surah === "string" ? surah : surah.slug;
  return ctx.kind === "arabic"
    ? surahPath(slug)
    : translationSurahPath(slug, ctx.lang, ctx.translator, 1);
};

export const surahLocalPagePathFor = (
  ctx: SurahRouteContext,
  surah: string | Pick<CatalogEntry, "slug">,
  localPage: number,
): `/app/${string}` => {
  const slug = typeof surah === "string" ? surah : surah.slug;
  return ctx.kind === "arabic"
    ? surahLocalPagePath(slug, localPage)
    : translationSurahPath(slug, ctx.lang, ctx.translator, localPage);
};

export const surahAyahPathFor = (
  ctx: SurahRouteContext,
  surah: Pick<CatalogEntry, "slug" | "num">,
  localPage: number,
  ayah: number,
): `/app/${string}` => `${surahLocalPagePathFor(ctx, surah, localPage)}#ayah-${surah.num}-${ayah}`;

export const globalPagePathFor = (
  ctx: SurahRouteContext,
  globalPage: number,
): `/app/${string}` =>
  ctx.kind === "arabic"
    ? `/app/page/${globalPage}`
    : translationGlobalPagePath(ctx.lang, ctx.translator, globalPage);

export const juzPathFor = (ctx: SurahRouteContext, n: number): `/app/${string}` =>
  ctx.kind === "arabic" ? `/app/juz/${n}` : translationJuzPath(ctx.lang, ctx.translator, n);

export const toArabicDigits = (n: number | string): string =>
  String(n).replace(/[0-9]/g, (d) => "٠١٢٣٤٥٦٧٨٩"[+d]);

export const surahMeta = (s: Pick<CatalogEntry, "place" | "ayahCount">): string =>
  `${s.place === "meccan" ? "Meccan" : "Medinan"} · ${s.ayahCount} verses`;

export const tafsirFor = (key: VerseKey): string => {
  const { num } = parseKey(key);
  return `Sample commentary for Surah ${num}, ${key} — in the full app this slot carries a short, credited tafsir summary.`;
};
