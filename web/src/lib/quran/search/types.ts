import type { Ayah } from "$lib/data/quran-types";

export interface SearchOpts {
  limit?: number;
  offset?: number;
}

export interface Highlight {
  /** UTF-16 offsets into the hit's exact display text. */
  start: number;
  end: number;
}

export const SearchHitKind = {
  Ayah: "ayah",
  Opener: "opener",
} as const;
export type SearchHitKind = (typeof SearchHitKind)[keyof typeof SearchHitKind];

export const SearchProvider = {
  Worker: "worker",
  Api: "api",
  Names: "names",
} as const;
export type SearchProvider = (typeof SearchProvider)[keyof typeof SearchProvider];

export interface AyahSearchHit {
  kind: typeof SearchHitKind.Ayah;
  /** Raw numbered ayah; matching and highlights use its canonical body. */
  ayah: Ayah;
  highlights: Highlight[];
}

export interface OpenerSearchHit {
  kind: typeof SearchHitKind.Opener;
  key: `opener:${number}`;
  surah: number;
  /** Navigation target only; the match is not attributed to this ayah. */
  anchorAyah: 1;
  text: string;
  highlights: Highlight[];
}

export type SearchHit = AyahSearchHit | OpenerSearchHit;

export interface SearchResponse {
  query: string;
  total: number;
  limit: number;
  offset: number;
  results: SearchHit[];
  /** Which engine answered. */
  source: SearchProvider;
}

export function searchHitKey(hit: SearchHit): string {
  return hit.kind === SearchHitKind.Opener ? hit.key : hit.ayah.key;
}

export function searchHitSurah(hit: SearchHit): number {
  return hit.kind === SearchHitKind.Opener ? hit.surah : hit.ayah.surah;
}

export function searchHitAnchorAyah(hit: SearchHit): number {
  return hit.kind === SearchHitKind.Opener ? hit.anchorAyah : hit.ayah.ayah;
}

export function searchHitText(hit: SearchHit): string {
  return hit.kind === SearchHitKind.Opener ? hit.text : hit.ayah.text;
}
