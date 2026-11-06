import { describe, expect, it } from "vite-plus/test";

import { verseKey } from "#lib/data/quran.js";
import type { NoteEntry } from "#lib/stores/annotations.svelte.js";

import { filterNotes, sortNotes } from "../notes-library.js";

function note(surah: number, ayah: number, text: string, updatedAt: number | null): NoteEntry {
  return {
    key: verseKey(surah, ayah),
    surah,
    ayah,
    text,
    updatedAt,
    sourceId: null,
  };
}

const NAMES = new Map([
  [1, "Al-Fatihah"],
  [2, "Al-Baqarah"],
]);
const surahName = (surah: number) => NAMES.get(surah) ?? `Surah ${surah}`;

const NOTES = [
  note(2, 255, "Ayat al-Kursi reflection", 300),
  note(1, 1, "Opening words", 100),
  note(2, 3, "Belief in the unseen", null),
];

describe("sortNotes", () => {
  it("orders by mushaf position for verse sort", () => {
    expect(sortNotes(NOTES, "verse").map((n) => n.key)).toEqual(["1:1", "2:3", "2:255"]);
  });

  it("orders recently edited first and treats untimed notes as oldest", () => {
    expect(sortNotes(NOTES, "newest").map((n) => n.key)).toEqual(["2:255", "1:1", "2:3"]);
  });

  it("orders least recently edited first", () => {
    expect(sortNotes(NOTES, "oldest").map((n) => n.key)).toEqual(["2:3", "1:1", "2:255"]);
  });

  it("keeps mushaf order for notes with equal timestamps", () => {
    const tied = [note(2, 5, "b", 50), note(2, 1, "a", 50)];
    expect(sortNotes(tied, "newest").map((n) => n.key)).toEqual(["2:1", "2:5"]);
  });

  it("does not mutate the input", () => {
    const input = [...NOTES];
    sortNotes(input, "newest");
    expect(input.map((n) => n.key)).toEqual(["2:255", "1:1", "2:3"]);
  });
});

describe("filterNotes", () => {
  it("returns everything for a blank query", () => {
    expect(filterNotes(NOTES, "   ", surahName)).toBe(NOTES);
  });

  it("matches note text case-insensitively", () => {
    expect(filterNotes(NOTES, "KURSI", surahName).map((n) => n.key)).toEqual(["2:255"]);
  });

  it("matches the verse reference", () => {
    expect(filterNotes(NOTES, "2:3", surahName).map((n) => n.key)).toEqual(["2:3"]);
  });

  it("matches the surah name", () => {
    expect(filterNotes(NOTES, "fatihah", surahName).map((n) => n.key)).toEqual(["1:1"]);
  });

  it("returns nothing when no field matches", () => {
    expect(filterNotes(NOTES, "zzz", surahName)).toEqual([]);
  });
});
