import { promises as fs } from "node:fs";
import os from "node:os";
import path from "node:path";

import {
  QuranScript,
  QuranSourceId,
  type QuranSourceId as QuranSourceIdValue,
} from "$lib/data/quran-types";
import { sourceProfile } from "$lib/quran/view/source-profiles";
import { afterEach, describe, expect, it, vi } from "vite-plus/test";

// The fail-soft reader seat (quran-sqlite.ts) resolves DB paths and quran-data.json
// from process.cwd(), so each scenario stages a temp cwd holding only the artifacts
// it needs — copies only; the provisioned db/ tree is never touched.
const webCwd = process.cwd();
const repoRoot = path.resolve(webCwd, "..");

const stagingDirectories: string[] = [];

async function stagingDirectory(): Promise<string> {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), "easyquran-ssr-fallback-"));
  stagingDirectories.push(directory);
  return directory;
}

async function stage(directory: string, sources: readonly QuranSourceIdValue[]): Promise<void> {
  await fs.mkdir(path.join(directory, "db/quran/arabic"), { recursive: true });
  await fs.mkdir(path.join(directory, "static/quran-meta"), { recursive: true });
  await fs.copyFile(
    path.join(webCwd, "static/quran-meta/quran-data.json"),
    path.join(directory, "static/quran-meta/quran-data.json"),
  );
  for (const source of sources) {
    await fs.copyFile(
      path.join(repoRoot, sourceProfile(source).artifact.repositoryPath),
      path.join(directory, sourceProfile(source).artifact.repositoryPath),
    );
  }
}

async function importSqliteModule(): Promise<typeof import("$lib/server/quran-sqlite")> {
  vi.resetModules();
  return await import("$lib/server/quran-sqlite");
}

afterEach(async () => {
  process.chdir(webCwd);
  vi.resetModules();
  await Promise.all(
    stagingDirectories
      .splice(0)
      .map((directory) => fs.rm(directory, { recursive: true, force: true })),
  );
});

describe("quran-sqlite fail-soft reader seat", () => {
  it("serves the plain Uthmani DB when the annotated artifact is missing, warning once", async () => {
    const directory = await stagingDirectory();
    await stage(directory, [QuranSourceId.TanzilUthmani]);
    process.chdir(directory);
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    try {
      const { readSurahText } = await importSqliteModule();

      const first = readSurahText(1);
      expect(first.sourceId).toBe(QuranSourceId.TanzilUthmani);
      expect(first.script).toBe(QuranScript.Uthmani);
      expect(first.verses).toHaveLength(7);

      // A read naming the demoted id explicitly serves the fallback seat too,
      // matching the worker's readerSeat mapping.
      const named = readSurahText(1, QuranSourceId.AnnotatedUthmani);
      expect(named.sourceId).toBe(QuranSourceId.TanzilUthmani);

      expect(warn).toHaveBeenCalledTimes(1);
      expect(warn.mock.calls[0]?.join(" ")).toContain("uthmani-annotated");
    } finally {
      warn.mockRestore();
    }
  });

  it("still throws when every pinned reader DB is missing", async () => {
    const directory = await stagingDirectory();
    await stage(directory, []);
    process.chdir(directory);
    const { readSurahText } = await importSqliteModule();
    expect(() => readSurahText(1)).toThrow(/\[quran-sqlite\] missing .* DB at /);
  });
});
