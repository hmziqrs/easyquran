import { readFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vite-plus/test";
import { QuranSourceId } from "$lib/data/quran-types";
import { decodeScript } from "$lib/quran/wire";

function read(rel: string): string {
  return readFileSync(path.resolve(process.cwd(), rel), "utf8");
}

describe("sha256 regression guard (docs/quran-system.md — Hard rules)", () => {
  it("published catalogues carry no sha256", () => {
    for (const rel of [
      "src/lib/data/translations.json",
      "../db/quran/tanzil/translations/index.min.json",
    ]) {
      expect(read(rel), `${rel} must not contain sha256`).not.toMatch(/sha256/i);
    }
  });

  const SHA_FREE = [
    "../rust/backend/api/src/quran/loader.rs",
    "../rust/backend/api/src/quran/store.rs",
    "../rust/backend/api/src/quran/view.rs",
    "../rust/backend/api/src/quran/translation_pool.rs",
    "../rust/backend/api/src/quran/mod.rs",
    "../rust/backend/api/src/modules/quran_v1/dto.rs",
    "src/lib/server/quran-sqlite.ts",
    "src/lib/server/quran-surah-page.ts",
    "src/lib/quran/view/source-profiles.ts",
    "src/lib/workers/download.ts",
    "src/lib/workers/opfs-cache.ts",
    "src/lib/workers/cached.ts",
    "src/lib/workers/quran.worker.ts",
    "src/lib/quran/manifest.ts",
    "src/lib/quran/wire.ts",
    "src/lib/config/site.ts",
    "src/lib/data/quran-types.ts",
    "src/lib/quran/catalogue.ts",
    "src/lib/offline/pack.ts",
    "scripts/gen-offline-pack.ts",
  ];
  it("automated Quran path source is sha-free (boot loaders, cache, wire, DTOs)", () => {
    for (const rel of SHA_FREE) {
      expect(read(rel), `${rel} must not compute/compare sha`).not.toMatch(
        /sha256|sha2|createHash|crypto\.subtle\.digest/i,
      );
    }
  });

  it("arabic registry + download path carry no sha256", () => {
    expect(read("src/lib/quran/view/source-profiles.ts")).not.toMatch(/sha256/i);
    expect(read("src/lib/workers/download.ts")).not.toMatch(/sha256/i);
  });

  it("artifact cache is keyed by id, not sha256", () => {
    const cache = read("src/lib/workers/opfs-cache.ts");
    expect(cache).toContain("opfs.get(spec.id,");
    expect(cache).toContain("opfs.put(spec.id,");
    expect(cache).toContain("tag: spec.id,");
  });

  it("/scripts ArtifactSpec carries no sha256 (decoder ignores stray sha)", () => {
    const decoded = decodeScript({
      id: QuranSourceId.TanzilUthmani,
      sizeBytes: 1,
      downloadUrl: "https://x/u",
    });
    expect(decoded).not.toBeNull();
    expect(decoded).not.toHaveProperty("sha256");
    const stray = decodeScript({
      id: QuranSourceId.TanzilUthmani,
      sizeBytes: 1,
      downloadUrl: "https://x/u",
      sha256: "deadbeef",
    });
    expect(stray).not.toBeNull();
    expect(stray).not.toHaveProperty("sha256");
  });
});
