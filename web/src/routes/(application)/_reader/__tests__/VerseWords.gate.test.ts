import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

import { describe, expect, it } from "vite-plus/test";

// Source-scan guard (F21): VerseRow must not pull the word engine in statically.
// Only the dynamic import of VerseWords.svelte is allowed, and it is gated on
// the registry being non-empty.

function webRoot(): string {
  let dir = process.cwd();
  for (let i = 0; i < 12; i += 1) {
    if (existsSync(resolve(dir, "src/app.html")) && existsSync(resolve(dir, "package.json"))) {
      return dir;
    }
    dir = resolve(dir, "..");
  }
  throw new Error("word gate guard: could not locate web/ root from " + process.cwd());
}

const VERSE_ROW = readFileSync(
  resolve(webRoot(), "src/routes/(application)/_reader/VerseRow.svelte"),
  "utf8",
);

describe("VerseRow word gate (F21)", () => {
  it("imports no word engine or view statically", () => {
    expect(VERSE_ROW).not.toMatch(/^\s*import[^;]*VerseWords/m);
    expect(VERSE_ROW).not.toMatch(/^\s*import[^;]*quran\/words\/(engine|tokens)/m);
  });

  it("loads the word view only behind the registry length check", () => {
    const gate = VERSE_ROW.indexOf("if (WORD_SOURCES.length === 0) return;");
    const load = VERSE_ROW.indexOf('import("./VerseWords.svelte")');
    expect(gate).toBeGreaterThan(-1);
    expect(load).toBeGreaterThan(gate);
  });

  it("keeps the plain text branch as the fallback", () => {
    expect(VERSE_ROW).toContain("{:else if Words !== null && wordSource !== undefined}");
  });
});
