/* Ambient declaration for the `quran-meta:data` virtual module emitted by
   vite-plugin-quran.ts at build/dev time. This file is a global script (no
   top-level import/export) so `declare module` declares — rather than
   augments — the virtual module, letting svelte-check type-check imports of
   CATALOG / NAVIGATION / SAJDAS without a physical source file. The shapes
   mirror quran-types.ts (kept in sync; the runtime values come from the plugin). */

declare module "quran-meta:data" {
  export interface CatalogEntry {
    num: number;
    slug: string;
    name: string;
    arabic: string;
    place: "meccan" | "medinan";
    ayahCount: number;
    revelationOrder: number;
    rukus: number;
    bismillah: "first-ayah" | "none" | "embedded-prefix";
    startGlobal: number;
  }
  export interface RangeEntry {
    index: number;
    startGlobal: number;
    endGlobal: number;
    first: string;
    last: string;
  }
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
    kind: "recommended" | "obligatory";
  }
  export const CATALOG: CatalogEntry[];
  export const NAVIGATION: NavigationData;
  export const SAJDAS: SajdaEntry[];
}
