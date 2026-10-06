import { existsSync } from "node:fs";
import path from "node:path";
import { DatabaseSync } from "node:sqlite";
import { pathToFileURL } from "node:url";

import { dev } from "$app/environment";
import { QuranScript } from "$lib/data/quran-types";
import { error } from "@sveltejs/kit";
import { clamp } from "es-toolkit";

import inventory from "../../../../../scripts/fonts/indopak/inventory.json";
import mapping from "../../../../../scripts/fonts/indopak/mapping.json";

export const prerender = false;

function databasePath(filename: string): string {
  const candidates = [
    path.resolve(process.cwd(), "db/quran/arabic", filename),
    path.resolve(process.cwd(), "../db/quran/arabic", filename),
  ];
  return (
    candidates.find((candidate) => existsSync(candidate)) ?? error(503, "Provision Quran DBs first")
  );
}

function openReadOnly(filename: string): DatabaseSync {
  const url = pathToFileURL(databasePath(filename));
  url.search = "?mode=ro&immutable=1";
  return new DatabaseSync(url, { readOnly: true });
}

function verse(database: DatabaseSync, key: string): string {
  const [sura, aya] = key.split(":").map((part) => Number(part));
  const row = database
    .prepare("SELECT text FROM quran_text WHERE sura = ? AND aya = ?")
    .get(sura!, aya!);
  if (!row) error(500, `Missing specimen verse ${key}`);
  return String(row.text);
}

function neighbor(database: DatabaseSync, surah: number, ayah: number) {
  const row = database
    .prepare("SELECT text FROM quran_text WHERE sura = ? AND aya = ?")
    .get(surah, ayah);
  if (!row) return null;
  return { key: `${surah}:${ayah}`, ayah, text: String(row.text) };
}

export function load({ url }: { url: URL }) {
  if (!dev) error(404, "Not found");
  const database = openReadOnly("quran-indopak.sqlite");
  const uthmani = openReadOnly("quran-uthmani.sqlite");
  try {
    const contexts = new Map<string, string[]>();
    for (const entry of mapping.entries) {
      for (const context of entry.contexts) {
        const labels = contexts.get(context.verse_key) ?? [];
        labels.push(`${entry.codepoint}: ${context.signature}`);
        contexts.set(context.verse_key, labels);
      }
    }
    for (const item of inventory.codepoints) {
      const key = item.representative_verse_keys[0];
      if (!key) continue;
      const labels = contexts.get(key) ?? [];
      labels.push(`${item.codepoint}: repertoire coverage`);
      contexts.set(key, labels);
    }
    for (const key of (url.searchParams.get("keys") ?? "").split(",")) {
      if (!/^\d{1,3}:\d{1,3}$/u.test(key)) continue;
      contexts.set(key, [...(contexts.get(key) ?? []), "Renderer regression"]);
    }
    const flowAudit = url.searchParams.get("audit") === "flow";
    const corpusAudit = url.searchParams.get("audit") === "corpus";
    const fullAudit = ["all", "flow", "corpus"].includes(url.searchParams.get("audit") ?? "");
    if (corpusAudit) {
      contexts.clear();
      const offset = clamp(
        Number.parseInt(url.searchParams.get("offset") ?? "0", 10) || 0,
        0,
        6236,
      );
      const limit = clamp(
        Number.parseInt(url.searchParams.get("limit") ?? "256", 10) || 256,
        1,
        512,
      );
      const rows = database
        .prepare('SELECT sura, aya FROM quran_text ORDER BY "index" LIMIT ? OFFSET ?')
        .all(limit, offset);
      for (const row of rows)
        contexts.set(`${Number(row.sura)}:${Number(row.aya)}`, ["Whole corpus verification"]);
    } else if (fullAudit) {
      const rows = database
        .prepare('SELECT sura, aya, text FROM quran_text ORDER BY "index"')
        .all();
      for (const row of rows) {
        const text = String(row.text);
        const codes = new Set<string>();
        for (const character of text) {
          if (/\p{Co}/u.test(character)) codes.add(character);
        }
        if (codes.size === 0) continue;
        const key = `${Number(row.sura)}:${Number(row.aya)}`;
        contexts.set(
          key,
          Array.from(
            codes,
            (character) => `U+${character.codePointAt(0)!.toString(16).toUpperCase()}`,
          ),
        );
      }
    }
    const specimens = [...contexts]
      .map(([key, labels]) => {
        const [surah, ayah] = key.split(":").map(Number);
        return {
          key,
          labels,
          text: verse(database, key),
          ayah: ayah!,
          previous: flowAudit ? neighbor(database, surah!, ayah! - 1) : null,
          next: flowAudit ? neighbor(database, surah!, ayah! + 1) : null,
        };
      })
      .filter((specimen) => !flowAudit || specimen.text.includes("\uE021"));
    const diagnosticKey = "17:7";
    const diagnosticText = verse(database, diagnosticKey);
    return {
      fullAudit,
      flowAudit,
      verseMode: url.searchParams.get("mode") === "verse",
      specimens,
      symbols: mapping.entries.map((entry) => ({
        codepoint: entry.codepoint,
        meaning: entry.meaning,
        occurrences: entry.occurrences,
      })),
      contextCount: mapping.entries.reduce((total, entry) => total + entry.contexts.length, 0),
      diagnostic: {
        key: diagnosticKey,
        original: diagnosticText,
        comparison: diagnosticText.replaceAll("\uE004", "\u0657"),
      },
      controls: [
        { key: "1:1", text: verse(database, "1:1"), script: QuranScript.IndoPak },
        { key: "112:1", text: verse(database, "112:1"), script: QuranScript.IndoPak },
        { key: "1:7", text: verse(uthmani, "1:7"), script: QuranScript.Uthmani },
      ],
    };
  } finally {
    database.close();
    uthmani.close();
  }
}
