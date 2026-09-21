import { browser } from "$app/environment";
import { deLocalizeUrl } from "$lib/paraglide/runtime";
import {
  globalPagePathFor,
  juzPathFor,
  surahLocalPagePathFor,
  type SurahRouteContext,
} from "$lib/data/quran";

/**
 * Position-preserving translation navigation, ported from the sidebar
 * TranslationPicker. Given the current reader pathname it extracts where the
 * reader is (surah local page / global page / juz), and hrefFor rebuilds the
 * same position for any target translation via the ctx-aware path helpers —
 * never a hand-built /app/ string (machine-guarded by nav-guard.test.ts).
 */
export type ReaderPosition =
  | { kind: "surah"; slug: string; localPage: number; lang?: string; translator?: string }
  | { kind: "globalPage"; n: number; lang?: string; translator?: string }
  | { kind: "juz"; n: number; lang?: string; translator?: string }
  | null;

export type TranslationTarget = { id: string; lang: string; translator: string };

function toNum(s: string | undefined): number {
  const n = s ? Number(s) : Number.NaN;
  return Number.isSafeInteger(n) && n > 0 ? n : 1;
}

export function positionOf(pathname: string): ReaderPosition {
  const segs = pathname.replace(/^\/app\/?/, "").split("/").filter(Boolean);
  if (segs.length === 0) return null;
  const tIdx = segs.indexOf("t");
  if (tIdx === -1) {
    if (segs[0] === "page") return segs[1] ? { kind: "globalPage", n: toNum(segs[1]) } : null;
    if (segs[0] === "juz") return segs[1] ? { kind: "juz", n: toNum(segs[1]) } : null;
    const slug = segs[0];
    if (!slug) return null;
    if (segs[1] === "page" && segs[2]) return { kind: "surah", slug, localPage: toNum(segs[2]) };
    return { kind: "surah", slug, localPage: 1 };
  }
  const lang = segs[tIdx + 1];
  const translator = segs[tIdx + 2];
  if (!lang || !translator) return null;
  const rest = segs.slice(tIdx + 3);
  if (tIdx === 1) {
    const slug = segs[0];
    if (!slug) return null;
    if (rest[0] === "page" && rest[1])
      return { kind: "surah", slug, localPage: toNum(rest[1]), lang, translator };
    return { kind: "surah", slug, localPage: 1, lang, translator };
  }
  if (rest[0] === "page" && rest[1])
    return { kind: "globalPage", n: toNum(rest[1]), lang, translator };
  if (rest[0] === "juz" && rest[1]) return { kind: "juz", n: toNum(rest[1]), lang, translator };
  return null;
}

/**
 * Where the reader ACTUALLY is, for position-preserving primary switches
 * (stress round 8, finding S1).
 *
 * SvelteKit 2.70.2 shallow routing does NOT update the page store url on
 * replaceState: SurahReader's scroll handler rewrites window.location to
 * /page/N while page.url still holds the bare surah slug, so a switch href
 * derived from the page store alone lands the reader back at local page 1
 * after a scrolled reading session. The live url cannot move while the
 * translations modal / reading-mode confirm dialog is open (scroll lock;
 * navigation closes the dialog), so reading it at open/confirm time is exact.
 *
 * Browser-only: reads positionOf(deLocalizeUrl(window.location.href)) FIRST
 * and returns it whenever it names a reader position. Everything else — SSR,
 * test environments whose window.location is not a reader route, non-reader
 * live urls — falls back to the passed page-store url (the juz/global-page
 * kinds never rewrite the url, so the fallback carries them).
 */
export function liveReaderPosition(fallbackUrl: URL): ReaderPosition {
  if (browser) {
    const live = positionOf(deLocalizeUrl(window.location.href).pathname);
    if (live !== null) return live;
  }
  return positionOf(deLocalizeUrl(fallbackUrl).pathname);
}

function ctxFor(target: TranslationTarget): SurahRouteContext {
  return { kind: "translation", lang: target.lang, translator: target.translator };
}

export function hrefFor(
  pos: ReaderPosition,
  target: TranslationTarget,
): `/app/${string}` | null {
  if (!pos) return null;
  const ctx = ctxFor(target);
  if (pos.kind === "surah") return surahLocalPagePathFor(ctx, pos.slug, pos.localPage);
  if (pos.kind === "globalPage") return globalPagePathFor(ctx, pos.n);
  return juzPathFor(ctx, pos.n);
}
