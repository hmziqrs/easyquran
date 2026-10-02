import { browser } from "$app/environment";
import {
  globalPagePathFor,
  hizbPathFor,
  juzPathFor,
  rubPathFor,
  surahPathFor,
  type SurahRouteContext,
} from "$lib/data/quran";
import { deLocalizeUrl } from "$lib/paraglide/runtime";

/**
 * Position-preserving translation navigation, ported from the sidebar
 * TranslationPicker. Given the current reader pathname it extracts where the
 * reader is (surah / global page / juz / hizb / rub), and hrefFor rebuilds the
 * same position for any target translation via the ctx-aware path helpers —
 * never a hand-built /app/ string (machine-guarded by nav-guard.test.ts).
 */
export type ReaderPosition =
  | { kind: "surah"; slug: string; lang?: string; translator?: string }
  | { kind: "globalPage"; n: number; lang?: string; translator?: string }
  | { kind: "juz"; n: number; lang?: string; translator?: string }
  | { kind: "hizb"; n: number; lang?: string; translator?: string }
  | { kind: "rub"; n: number; lang?: string; translator?: string }
  | null;

export type TranslationTarget = { id: string; lang: string; translator: string };

function toNum(s: string | undefined): number {
  const n = s ? Number(s) : Number.NaN;
  return Number.isSafeInteger(n) && n > 0 ? n : 1;
}

const RESERVED_SINGLE_SEGMENTS = new Set([
  "app",
  "en",
  "juz",
  "page",
  "hizb",
  "rub",
  "t",
  "surah",
  "pages",
  "yours",
  "search",
  "settings",
  "bookmarks",
]);

export function positionOf(pathname: string): ReaderPosition {
  // Scheme A: de-localize then optional /ar strip; the live reader path is
  // already prefix-less for en. (Callers pass deLocalizeUrl output or a live
  // URL from liveReaderPosition, which de-localizes itself.)
  const segs = pathname
    .replace(/^\/ar(?=\/)/, "")
    .replace(/^\//, "")
    .split("/")
    .filter(Boolean);
  if (segs.length === 0) return null;
  const tIdx = segs.indexOf("t");
  if (tIdx === -1) {
    if (segs[0] === "page") return segs[1] ? { kind: "globalPage", n: toNum(segs[1]) } : null;
    if (segs[0] === "juz") return segs[1] ? { kind: "juz", n: toNum(segs[1]) } : null;
    if (segs[0] === "hizb") return segs[1] ? { kind: "hizb", n: toNum(segs[1]) } : null;
    if (segs[0] === "rub") return segs[1] ? { kind: "rub", n: toNum(segs[1]) } : null;
    const slug = segs[0];
    if (!slug) return null;
    // Reserved words are indexes/product pages/the dead /app marker — never a
    // reader position (the single-segment range kinds above need their /N).
    if (RESERVED_SINGLE_SEGMENTS.has(slug)) return null;
    return { kind: "surah", slug };
  }
  const lang = segs[tIdx + 1];
  const translator = segs[tIdx + 2];
  if (!lang || !translator) return null;
  const rest = segs.slice(tIdx + 3);
  // tIdx === 1: the /t/ tree hangs off a surah slug (…/slug/t/lang/translator);
  // any other t position is the top-level range tree (/app/t/lang/translator/…).
  if (tIdx === 1) {
    const slug = segs[0];
    if (!slug) return null;
    return { kind: "surah", slug, lang, translator };
  }
  if (rest[0] === "page" && rest[1])
    return { kind: "globalPage", n: toNum(rest[1]), lang, translator };
  if (rest[0] === "juz" && rest[1]) return { kind: "juz", n: toNum(rest[1]), lang, translator };
  if (rest[0] === "hizb" && rest[1]) return { kind: "hizb", n: toNum(rest[1]), lang, translator };
  if (rest[0] === "rub" && rest[1]) return { kind: "rub", n: toNum(rest[1]), lang, translator };
  return null;
}

/**
 * Where the reader ACTUALLY is, for position-preserving primary switches.
 * The path never moves while scrolling (one URL per surah), so live and
 * page-store urls only diverge by fragment/query — reading the live url first
 * still carries a fresh ?v= position into the switch, and browser-only callers
 * keep exactness when the modal is open over a scrolled reader.
 *
 * Browser-only: reads positionOf(deLocalizeUrl(window.location.href)) FIRST
 * and returns it whenever it names a reader position. Everything else — SSR,
 * test environments whose window.location is not a reader route, non-reader
 * live urls — falls back to the passed page-store url.
 */
export function liveReaderPosition(fallbackUrl: URL): ReaderPosition {
  if (browser) {
    const live = positionOf(deLocalizeUrl(window.location.href).pathname);
    if (live !== null) return live;
  }
  return positionOf(deLocalizeUrl(fallbackUrl).pathname);
}

/** The Arabic route at the same reader position (Reading → Arabic from a translation page). */
export function arabicHrefFor(pos: ReaderPosition): `/${string}` | null {
  if (!pos) return null;
  const ctx: SurahRouteContext = { kind: "arabic" };
  if (pos.kind === "surah") return surahPathFor(ctx, pos.slug);
  if (pos.kind === "globalPage") return globalPagePathFor(ctx, pos.n);
  if (pos.kind === "juz") return juzPathFor(ctx, pos.n);
  if (pos.kind === "hizb") return hizbPathFor(ctx, pos.n);
  return rubPathFor(ctx, pos.n);
}

function ctxFor(target: TranslationTarget): SurahRouteContext {
  return { kind: "translation", lang: target.lang, translator: target.translator };
}

export function hrefFor(pos: ReaderPosition, target: TranslationTarget): `/${string}` | null {
  if (!pos) return null;
  const ctx = ctxFor(target);
  if (pos.kind === "surah") return surahPathFor(ctx, pos.slug);
  if (pos.kind === "globalPage") return globalPagePathFor(ctx, pos.n);
  if (pos.kind === "juz") return juzPathFor(ctx, pos.n);
  if (pos.kind === "hizb") return hizbPathFor(ctx, pos.n);
  return rubPathFor(ctx, pos.n);
}
