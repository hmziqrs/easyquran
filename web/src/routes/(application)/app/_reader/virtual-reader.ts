import type { Ayah, SurahLocalPageData } from "$lib/data/quran-types";
import { clamp } from "es-toolkit";

export interface ReaderVirtualItem {
  key: string;
  localPage: number;
  estimate: number;
  verseKey?: string;
}

export interface AyahRenderItem extends ReaderVirtualItem {
  kind: "ayah";
  ayah: Ayah;
  pageData: SurahLocalPageData;
}

export interface PageRenderItem extends ReaderVirtualItem {
  kind: "page";
  pageData: SurahLocalPageData;
}

export type ReaderRenderItem = AyahRenderItem | PageRenderItem;

export function bufferedIndexes(start: number, end: number, count: number, pinned = -1): number[] {
  if (count <= 0) return [];
  const first = clamp(start, 0, count - 1);
  const last = clamp(end, first, count - 1);
  const indexes = Array.from({ length: last - first + 1 }, (_, index) => first + index);
  if (pinned >= 0 && pinned < count && (pinned < first || pinned > last)) {
    indexes.push(pinned);
    indexes.sort((a, b) => a - b);
  }
  return indexes;
}

export function estimateTextHeight(
  text: string,
  fontSize: number,
  width: number,
  lineHeight: number,
): number {
  const characters = text.replace(/[\u0610-\u061a\u064b-\u065f\u0670\u06d6-\u06ed]/g, "").length;
  const perLine = Math.max(12, Math.max(width, 320) / (fontSize * 0.55));
  return Math.max(1, Math.ceil(characters / perLine)) * fontSize * lineHeight;
}
