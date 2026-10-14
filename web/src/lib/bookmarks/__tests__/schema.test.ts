import { describe, expect, it } from "vite-plus/test";

import {
  compareFolders,
  decodeBookmark,
  decodeBookmarkFolder,
  decodeBookmarksEnvelope,
  defaultFolderIdFor,
  isSingleVerseBookmark,
  liveKeyOf,
  type BookmarkFolder,
} from "../schema";

const F = "f1f1f1f1-f1f1-4f1f-8f1f-f1f1f1f1f1f1";
const B = "b1b1b1b1-b1b1-4b1b-8b1b-b1b1b1b1b1b1";
const TS = "2026-10-14T00:00:00.000Z";

describe("default folder ids", () => {
  it("match the server's UUIDv5 derivation byte for byte", () => {
    // Same vectors as uuid.uuid5(NS, name) — and Rust `Uuid::new_v5`.
    expect(defaultFolderIdFor(1)).toBe("9c314774-c392-5a9a-b569-79b8e7e31226");
    expect(defaultFolderIdFor("local")).toBe("e520c7f8-36a1-5ef3-8a07-7a2c672d0d76");
  });
});

describe("decoders", () => {
  it("decodes a folder, flagging the default", () => {
    expect(
      decodeBookmarkFolder({ id: F, name: "Hifz", isDefault: true, createdAt: TS, updatedAt: TS }),
    ).toEqual({
      id: F,
      name: "Hifz",
      isDefault: true,
      position: null,
      createdAt: TS,
      updatedAt: TS,
    });
    expect(
      decodeBookmarkFolder({ id: "short", name: "x", createdAt: TS, updatedAt: TS }),
    ).toBeNull();
    expect(decodeBookmarkFolder({ id: F, name: "", createdAt: TS, updatedAt: TS })).toBeNull();
  });

  it("decodes a bookmark; every row needs a folder and ranges travel as a pair", () => {
    const row = { id: B, folderId: F, surah: 2, ayah: 285, createdAt: TS, updatedAt: TS };
    expect(decodeBookmark(row)).toMatchObject({ endSurah: null, endAyah: null, sourceId: null });
    expect(decodeBookmark({ ...row, endSurah: 2, endAyah: 286 })).toMatchObject({ endAyah: 286 });
    expect(decodeBookmark({ ...row, endSurah: 2 })).toBeNull();
    expect(decodeBookmark({ ...row, folderId: null })).toBeNull();
    expect(decodeBookmark({ ...row, surah: 115 })).toBeNull();
  });

  it("drops malformed rows instead of failing the whole snapshot", () => {
    const envelope = decodeBookmarksEnvelope({
      applied: 1,
      folders: [{ id: F, name: "A", createdAt: TS, updatedAt: TS }, { nope: true }],
      bookmarks: [{ id: B, folderId: F, surah: 1, ayah: 1, createdAt: TS, updatedAt: TS }, null],
    });
    expect(envelope?.folders).toHaveLength(1);
    expect(envelope?.bookmarks).toHaveLength(1);
    expect(envelope?.skipped).toBe(0);
    expect(decodeBookmarksEnvelope({ folders: [] })).toBeNull();
  });
});

describe("keys and order", () => {
  it("keys rows by folder + start verse + range end", () => {
    const single = { folderId: F, surah: 2, ayah: 285, endSurah: null, endAyah: null };
    const range = { ...single, endSurah: 2, endAyah: 286 };
    expect(isSingleVerseBookmark(single)).toBe(true);
    expect(isSingleVerseBookmark(range)).toBe(false);
    expect(liveKeyOf(single)).not.toBe(liveKeyOf(range));
    expect(liveKeyOf(single)).not.toBe(liveKeyOf({ ...single, folderId: B }));
  });

  it("sorts the default folder first, then manual position, then name", () => {
    const folder = (name: string, position: number | null, isDefault = false): BookmarkFolder => ({
      id: name,
      name,
      isDefault,
      position,
      createdAt: TS,
      updatedAt: TS,
    });
    const sorted = [
      folder("b", null),
      folder("z", 0),
      folder("Bookmarks", null, true),
      folder("a", null),
    ]
      .sort((x, y) => compareFolders(x, y))
      .map((f) => f.name);
    expect(sorted).toEqual(["Bookmarks", "z", "a", "b"]);
  });
});
