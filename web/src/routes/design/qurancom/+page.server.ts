import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { DatabaseSync } from "node:sqlite";
import { pathToFileURL } from "node:url";

import { dev } from "$app/env";
import { error } from "@sveltejs/kit";
import { clamp } from "es-toolkit";

import {
  decodeMirrorVerses,
  diffSummary,
  mirrorBareText,
  ourBareText,
  QURANCOM_PAIRS,
  type DiffSummary,
  type QurancomPair,
} from "./compare.ts";

export const prerender = false;

const DEFAULT_VERSES = "13:26,13:27,13:28,13:29,1:1,2:255,114:6";
const MAX_VERSES = 12;

function mirrorScriptPath(file: string): string {
  const candidates = [
    path.resolve(process.cwd(), "web/static/qurancom/scripts", file),
    path.resolve(process.cwd(), "static/qurancom/scripts", file),
  ];
  return (
    candidates.find((candidate) => existsSync(candidate)) ??
    error(503, "quran.com mirror missing — run bun run quran:mirror")
  );
}

function databasePath(file: string): string | null {
  const candidates = [
    path.resolve(process.cwd(), "db/quran/arabic", file),
    path.resolve(process.cwd(), "../db/quran/arabic", file),
  ];
  return candidates.find((candidate) => existsSync(candidate)) ?? null;
}

function openReadOnly(absolute: string): DatabaseSync {
  const url = pathToFileURL(absolute);
  url.search = "?mode=ro&immutable=1";
  return new DatabaseSync(url, { readOnly: true });
}

function readVerse(database: DatabaseSync, key: string): string | null {
  const [sura, aya] = key.split(":").map((part) => Number(part));
  const row = database
    .prepare("SELECT text FROM quran_text WHERE sura = ? AND aya = ?")
    .get(sura ?? 0, aya ?? 0);
  if (!row) return null;
  return String(row.text);
}

function parseVerses(param: string | null): string[] {
  const source = param?.trim() ? param : DEFAULT_VERSES;
  const keys: string[] = [];
  for (const part of source.split(",")) {
    const match = /^(\d{1,3}):(\d{1,3})$/u.exec(part.trim());
    if (!match) continue;
    const sura = clamp(Number(match[1] ?? 0), 1, 114);
    const aya = clamp(Number(match[2] ?? 0), 1, 286);
    const key = `${sura}:${aya}`;
    if (!keys.includes(key)) keys.push(key);
    if (keys.length >= MAX_VERSES) break;
  }
  if (keys.length === 0) return DEFAULT_VERSES.split(",");
  return keys;
}

interface OurRow {
  readonly id: string;
  readonly label: string;
  readonly present: boolean;
  readonly raw: string | null;
  readonly bare: string | null;
  readonly diff: DiffSummary | null;
  readonly note: string | null;
}

function pairView(pair: QurancomPair, verseKeys: readonly string[]) {
  const mirror = decodeMirrorVerses(
    readFileSync(mirrorScriptPath(pair.mirrorFile), "utf8"),
    pair.mirrorField,
  );
  const databases = pair.ours.map((our) => {
    const found = databasePath(our.dbFile);
    return { our, database: found === null ? null : openReadOnly(found) };
  });
  try {
    const verses = verseKeys.flatMap((key) => {
      const mirrorRaw = mirror.get(key);
      if (mirrorRaw === undefined) return [];
      const mirrorBare = mirrorBareText(pair, mirrorRaw);
      const rows = databases.map(({ our, database }): OurRow => {
        if (database === null) {
          return {
            id: our.id,
            label: our.label,
            present: false,
            raw: null,
            bare: null,
            diff: null,
            note: `DB not provisioned (${our.dbFile}) — run just quran-fetch`,
          };
        }
        const raw = readVerse(database, key);
        if (raw === null) {
          return {
            id: our.id,
            label: our.label,
            present: false,
            raw: null,
            bare: null,
            diff: null,
            note: `verse ${key} not found in ${our.dbFile}`,
          };
        }
        const bare = ourBareText(pair, raw);
        return {
          id: our.id,
          label: our.label,
          present: true,
          raw,
          bare,
          diff: diffSummary(mirrorBare, bare),
          note: null,
        };
      });
      return [{ key, mirrorRaw, mirrorBare, rows }];
    });
    return {
      slug: pair.slug,
      label: pair.label,
      mirrorFile: pair.mirrorFile,
      bare: pair.bare,
      mirrorFont: pair.mirrorFont,
      ourFont: pair.ourFont,
      verses,
    };
  } finally {
    for (const { database } of databases) database?.close();
  }
}

export function load({ url }: { url: URL }) {
  if (!dev) error(404, "Not found");
  const requested = url.searchParams.get("script") ?? "all";
  const selected =
    requested === "all" ? QURANCOM_PAIRS : QURANCOM_PAIRS.filter((pair) => pair.slug === requested);
  const pairs = selected.length > 0 ? selected : QURANCOM_PAIRS;
  const verseKeys = parseVerses(url.searchParams.get("verses"));
  return {
    requestedScript: requested,
    versesParam: verseKeys.join(","),
    scripts: QURANCOM_PAIRS.map((pair) => ({ slug: pair.slug, label: pair.label })),
    pairs: pairs.map((pair) => pairView(pair, verseKeys)),
  };
}
