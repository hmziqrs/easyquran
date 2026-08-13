import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { createQuranData } from "$lib/data/quran-data";
import type { CatalogEntry, SurahLink, SurahRenderMetadata } from "$lib/data/quran-types";

const roots = [
  path.resolve(process.cwd(), "static/quran-meta"),
  path.resolve(process.cwd(), "web/static/quran-meta"),
  path.resolve(process.cwd(), "build/client/quran-meta"),
];
const dataPath = roots.map((root) => path.join(root, "quran-data.json")).find((candidate) => existsSync(candidate));
if (!dataPath) throw new Error("[quran-data] missing quran-data.json");

export const QURAN_DATA = createQuranData(JSON.parse(readFileSync(dataPath, "utf8")));

export function toSurahRenderMetadata(entry: CatalogEntry): SurahRenderMetadata {
  const { num, slug, name, arabic, place, ayahCount } = entry;
  return { num, slug, name, arabic, place, ayahCount };
}

export function toSurahLink(entry: CatalogEntry): SurahLink {
  const { num, slug, name, arabic } = entry;
  return { num, slug, name, arabic };
}
