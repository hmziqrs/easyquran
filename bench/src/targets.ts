import { writeFileSync } from "node:fs";
import { SEED, WEB_ORIGIN } from "./config.ts";
import { loadKeyspace, segmentsOf, type Keyspace } from "./keyspace.ts";
import { Zipf, rng } from "./zipf.ts";

const TRANSLATION_ALPHA = 1.0;
const SURAH_ALPHA = 0.8;
const FIRST_PAGE_SHARE = 0.55;
/** Cap on distinct lines written per stage — vegeta cycles the file when it runs out. */
const MAX_TARGET_LINES = 60_000;

/** HTML suites only: this benchmark measures SSR and prerendered page delivery, nothing else. */
export const SUITES = ["translated-surah", "arabic-prerendered"] as const;
export type Suite = (typeof SUITES)[number];

export interface TargetSet {
  readonly file: string;
  readonly urls: readonly string[];
  /** Distinct cache keys the stream touches — what `warm` primes. */
  readonly distinct: readonly string[];
}

function translatedSurahUrl(id: string, slug: string, page: number): string {
  const { lang, translator } = segmentsOf(id);
  const base = `/app/${slug}/t/${lang}/${translator}`;
  return page > 1 ? `${base}/page/${page}` : base;
}

/** Prerendered Arabic HTML: served off disk by adapter-node, no SSR, no upstream call. */
function arabicUrl(ks: Keyspace, sIndex: number, roll: number, page: number): string {
  const surah = ks.surahs[sIndex]!;
  if (roll < 0.7) return page > 1 ? `/app/${surah.slug}/page/${page}` : `/app/${surah.slug}`;
  if (roll < 0.9) return `/app/page/${1 + Math.floor((roll - 0.7) * 5 * 604)}`;
  return `/app/juz/${1 + Math.floor((roll - 0.9) * 10 * 30)}`;
}

/**
 * Builds one stage's request stream. Deterministic in `seed`, so every runtime replays the exact
 * same key order — this is what makes the comparison an A/B rather than three experiments.
 */
export function buildTargets(suite: string, count: number, file: string, seed = SEED): TargetSet {
  if (!SUITES.includes(suite as Suite)) throw new Error(`[targets] unsupported suite ${suite}`);
  const ks = loadKeyspace();
  const usable = ks.translations.filter((id) => id.includes("."));
  const translations = new Zipf(usable.length, TRANSLATION_ALPHA, seed);
  const surahs = new Zipf(ks.surahs.length, SURAH_ALPHA, seed ^ 0x9e3779b9);
  const coin = rng(seed ^ 0x85ebca6b);

  const lines = Math.min(count, MAX_TARGET_LINES);
  const urls: string[] = [];
  const distinct = new Set<string>();

  for (let i = 0; i < lines; i += 1) {
    const sIndex = surahs.sample();
    const surah = ks.surahs[sIndex]!;
    // Readers open at the top and drift down: page 1 dominates, the rest share the tail.
    const page =
      surah.localPages === 1 || coin() < FIRST_PAGE_SHARE
        ? 1
        : 2 + Math.floor(coin() * (surah.localPages - 1));
    const url =
      suite === "translated-surah"
        ? translatedSurahUrl(usable[translations.sample()]!, surah.slug, page)
        : arabicUrl(ks, sIndex, coin(), page);
    urls.push(url);
    distinct.add(url);
  }

  writeFileSync(file, `${urls.map((url) => `GET ${WEB_ORIGIN}${url}`).join("\n")}\n`, "utf8");
  return { file, urls, distinct: [...distinct] };
}
