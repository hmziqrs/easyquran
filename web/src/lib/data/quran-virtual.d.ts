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
    openerKind: "verse" | "header" | "none";
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
