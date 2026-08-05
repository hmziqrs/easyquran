import { readFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vite-plus/test";

function read(rel: string): string {
  return readFileSync(path.resolve(process.cwd(), rel), "utf8");
}

describe("sha256 regression guard (docs/quran.md §2)", () => {
  it("published catalogues carry no sha256", () => {
    for (const rel of [
      "src/lib/data/translations.json",
      "../db/quran/tanzil/translations/index.min.json",
    ]) {
      expect(read(rel), `${rel} must not contain sha256`).not.toMatch(/sha256/i);
    }
  });

  it("artifact cache is keyed by id, not sha256", () => {
    const cache = read("src/lib/workers/opfs-cache.ts");
    expect(cache).toContain("opfs.get(spec.id,");
    expect(cache).toContain("opfs.put(spec.id,");
    expect(cache).toContain("tag: spec.id,");
  });
});
