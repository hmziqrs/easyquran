import { existsSync } from "node:fs";
import path from "node:path";
import { DatabaseSync } from "node:sqlite";

import { dev } from "$app/environment";
import { QuranScript } from "$lib/data/quran-types";
import { error } from "@sveltejs/kit";

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

function verse(database: DatabaseSync, key: string): string {
  const [sura, aya] = key.split(":").map((part) => Number(part));
  const row = database
    .prepare("SELECT text FROM quran_text WHERE sura = ? AND aya = ?")
    .get(sura!, aya!);
  if (!row) error(500, `Missing specimen verse ${key}`);
  return String(row.text);
}

export function load() {
  if (!dev) error(404, "Not found");
  const database = new DatabaseSync(databasePath("quran-indopak.sqlite"), { readOnly: true });
  const uthmani = new DatabaseSync(databasePath("quran-uthmani.sqlite"), { readOnly: true });
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
    const specimens = [...contexts].map(([key, labels]) => ({
      key,
      labels,
      text: verse(database, key),
      ayah: Number(key.split(":")[1]),
    }));
    const diagnosticKey = "17:7";
    const diagnosticText = verse(database, diagnosticKey);
    return {
      specimens,
      symbols: mapping.entries.map((entry) => ({
        codepoint: entry.codepoint,
        meaning: entry.meaning,
        occurrences: entry.occurrences,
      })),
      contextCount: mapping.entries.reduce((total, entry) => total + entry.contexts.length, 0),
      blocker: mapping.renderer_blocker,
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
