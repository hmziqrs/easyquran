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
