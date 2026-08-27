import { describe, expect, it } from "vite-plus/test";

import {
  AYAH_MAX,
  FOLDER_NAME_MAX,
  decodeBookmark,
  decodeBookmarkFolder,
  decodeBookmarksEnvelope,
  parseVerseKey,
  verseKeyOf,
} from "../schema";

/** JSON scalar a wire fixture may carry; decoders must reject every non-string/number shape. */
type Wire = string | number | boolean | null;

function folderWire(overrides: Partial<Record<"id" | "name" | "createdAt" | "updatedAt", Wire>> = {}) {
  return {
    id: "f1",
    name: "Ramadan",
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-02T00:00:00Z",
    ...overrides,
  };
}

function bookmarkWire(
  overrides: Partial<Record<"id" | "folderId" | "surah" | "ayah" | "createdAt" | "updatedAt", Wire>> = {},
) {
  return {
    id: "b1",
    folderId: null,
    surah: 2,
    ayah: 255,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-02T00:00:00Z",
    ...overrides,
  };
}

describe("verseKeyOf / parseVerseKey", () => {
  it("round-trips the legacy `${surah}:${ayah}` format", () => {
    expect(verseKeyOf(1, 1)).toBe("1:1");
    expect(verseKeyOf(2, 255)).toBe("2:255");
    expect(verseKeyOf(114, 6)).toBe("114:6");
    for (const key of ["1:1", "2:255", "114:6"]) {
      expect(parseVerseKey(key)).toEqual({
        surah: Number(key.split(":")[0]),
        ayah: Number(key.split(":")[1]),
      });
    }
  });

  it("rejects malformed keys instead of defaulting", () => {
    for (const bad of ["", "2", ":255", "2:", "a:b", "2:255x", "2:25:5", " 2:255"]) {
      expect(parseVerseKey(bad), bad).toBeNull();
    }
  });
});

describe("decodeBookmarkFolder", () => {
  it("decodes a valid row", () => {
    expect(decodeBookmarkFolder(folderWire())).toEqual({
      id: "f1",
      name: "Ramadan",
      createdAt: "2026-01-01T00:00:00Z",
      updatedAt: "2026-01-02T00:00:00Z",
    });
  });

  it("rejects malformed rows", () => {
    expect(decodeBookmarkFolder(null)).toBeNull();
    expect(decodeBookmarkFolder("nope")).toBeNull();
    expect(decodeBookmarkFolder(folderWire({ id: "" }))).toBeNull();
    expect(decodeBookmarkFolder(folderWire({ name: "" }))).toBeNull();
    expect(decodeBookmarkFolder(folderWire({ name: "x".repeat(FOLDER_NAME_MAX + 1) }))).toBeNull();
    expect(decodeBookmarkFolder(folderWire({ createdAt: 42 }))).toBeNull();
    expect(decodeBookmarkFolder(folderWire({ updatedAt: undefined }))).toBeNull();
  });
});

describe("decodeBookmark", () => {
  it("decodes a row with folderId null and with a folder id", () => {
    expect(decodeBookmark(bookmarkWire())).toEqual({
      id: "b1",
      folderId: null,
      surah: 2,
      ayah: 255,
      createdAt: "2026-01-01T00:00:00Z",
      updatedAt: "2026-01-02T00:00:00Z",
    });
    expect(decodeBookmark(bookmarkWire({ folderId: "f1" }))?.folderId).toBe("f1");
  });

  it("rejects out-of-range or non-integer coordinates and bad folderId", () => {
    expect(decodeBookmark(bookmarkWire({ surah: 0 }))).toBeNull();
    expect(decodeBookmark(bookmarkWire({ surah: 115 }))).toBeNull();
    expect(decodeBookmark(bookmarkWire({ ayah: 0 }))).toBeNull();
    expect(decodeBookmark(bookmarkWire({ ayah: AYAH_MAX + 1 }))).toBeNull();
    expect(decodeBookmark(bookmarkWire({ surah: 2.5 }))).toBeNull();
    expect(decodeBookmark(bookmarkWire({ folderId: 42 }))).toBeNull();
    expect(decodeBookmark(bookmarkWire({ folderId: "" }))).toBeNull();
    expect(decodeBookmark(bookmarkWire({ id: "" }))).toBeNull();
    expect(decodeBookmark(bookmarkWire({ updatedAt: null }))).toBeNull();
  });
});

describe("decodeBookmarksEnvelope", () => {
  it("decodes counts, folders and bookmarks", () => {
    const envelope = decodeBookmarksEnvelope({
      applied: 3,
      skipped: 1,
      folders: [folderWire()],
      bookmarks: [bookmarkWire(), bookmarkWire({ id: "b2" })],
    });
    expect(envelope).not.toBeNull();
    expect(envelope?.applied).toBe(3);
    expect(envelope?.skipped).toBe(1);
    expect(envelope?.folders).toHaveLength(1);
    expect(envelope?.bookmarks).toHaveLength(2);
  });

  it("drops malformed rows instead of failing the whole snapshot", () => {
    const envelope = decodeBookmarksEnvelope({
      applied: 2,
      skipped: 0,
      folders: [folderWire(), folderWire({ id: 7 })],
      bookmarks: [bookmarkWire(), bookmarkWire({ surah: 999 })],
    });
    expect(envelope?.folders).toHaveLength(1);
    expect(envelope?.bookmarks).toHaveLength(1);
  });

  it("rejects bodies without both arrays or with garbage counts", () => {
    expect(decodeBookmarksEnvelope(null)).toBeNull();
    expect(decodeBookmarksEnvelope("nope")).toBeNull();
    expect(decodeBookmarksEnvelope({ folders: [] })).toBeNull();
    expect(decodeBookmarksEnvelope({ bookmarks: [] })).toBeNull();
    expect(decodeBookmarksEnvelope({ applied: "3", skipped: 0, folders: [], bookmarks: [] })?.applied)
      .toBe(0);
  });
});
