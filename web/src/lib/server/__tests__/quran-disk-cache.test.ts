import { promises as fs } from "node:fs";
import os from "node:os";
import path from "node:path";

import { version as appBuildId } from "$app/environment";
import { diskCacheKey, QuranDiskCache } from "$lib/server/quran-disk-cache";
import { afterEach, describe, expect, it } from "vite-plus/test";

const directories: string[] = [];

async function temporaryDirectory(): Promise<string> {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), "easyquran-ssr-cache-"));
  directories.push(directory);
  return directory;
}

afterEach(async () => {
  await Promise.all(
    directories.splice(0).map((directory) => fs.rm(directory, { recursive: true })),
  );
});

describe("Quran translated-page disk cache", () => {
  it("namespaces derived HTML by web build without changing the Quran source id", () => {
    expect(diskCacheKey("en.sahih", "surah", 2, 1)).toBe(
      `build-${appBuildId}__en.sahih__surah__2__1`,
    );
  });

  it("records a cold miss, writes HTML atomically, then serves a warm hit", async () => {
    const cache = new QuranDiskCache({
      directory: await temporaryDirectory(),
      ttlMs: 60_000,
      budgetBytes: 1_024,
    });

    expect(await cache.get("en.sahih__surah__2__1")).toBeNull();
    await cache.set("en.sahih__surah__2__1", "<html>translated</html>");
    expect(await cache.get("en.sahih__surah__2__1")).toBe("<html>translated</html>");
    expect(await cache.stats()).toMatchObject({
      hits: 1,
      misses: 1,
      writes: 1,
      entries: 1,
      errors: 0,
    });
  });

  it("expires entries after seven-day-style TTL without serving stale HTML", async () => {
    let now = Date.now();
    const cache = new QuranDiskCache({
      directory: await temporaryDirectory(),
      ttlMs: 100,
      budgetBytes: 1_024,
      now: () => now,
    });

    await cache.set("en.sahih__surah__2__1", "fresh");
    now += 101;
    expect(await cache.get("en.sahih__surah__2__1")).toBeNull();
    expect(await cache.stats()).toMatchObject({ entries: 0, evictions: 1 });
  });

  it("touches warm entries and evicts least-recently-used HTML to enforce disk budget", async () => {
    let now = Date.now();
    const cache = new QuranDiskCache({
      directory: await temporaryDirectory(),
      ttlMs: 60_000,
      budgetBytes: 12,
      now: () => now,
    });

    await cache.set("a", "aaaaaa");
    now += 10;
    await cache.set("b", "bbbbbb");
    now += 10;
    expect(await cache.get("a")).toBe("aaaaaa");
    now += 10;
    await cache.set("c", "cccccc");

    expect(await cache.get("b")).toBeNull();
    expect(await cache.get("a")).toBe("aaaaaa");
    expect(await cache.get("c")).toBe("cccccc");
    expect(await cache.stats()).toMatchObject({ entries: 2, bytes: 12, evictions: 1 });
  });
});
