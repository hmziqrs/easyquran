import { readFileSync } from "node:fs";
import path from "node:path";
import { beforeEach, describe, expect, it, vi } from "vite-plus/test";
import { loadQuranData, resetQuranDataForTests } from "$lib/data/quran-data-client";

const snapshot = readFileSync(
  path.resolve(process.cwd(), "static/quran-meta/quran-data.json"),
  "utf8",
);

describe("browser Quran data loader", () => {
  beforeEach(() => {
    resetQuranDataForTests();
    vi.unstubAllGlobals();
  });

  it("does not fetch merely because its module was imported", () => {
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("fetches the one immutable snapshot at most once, on demand", async () => {
    const fetchMock = vi.fn(async (input: string | URL | Request) => {
      const url = typeof input === "string" ? input : input instanceof URL ? input.href : input.url;
      const found = url.endsWith("quran-data.json");
      return new Response(found ? snapshot : "not found", {
        status: found ? 200 : 404,
        headers: { "content-type": "application/json" },
      });
    });
    vi.stubGlobal("fetch", fetchMock);

    const [first, second] = await Promise.all([loadQuranData(), loadQuranData()]);
    expect(first).toBe(second);
    expect(first.surahs).toHaveLength(114);
    expect(first.rangeCount("page")).toBe(604);
    expect(fetchMock).toHaveBeenCalledTimes(1);

    expect(await loadQuranData()).toBe(first);
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it("allows a failed request to be retried", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(new Response("unavailable", { status: 503 }))
      .mockResolvedValueOnce(
        new Response(snapshot, {
          status: 200,
          headers: { "content-type": "application/json" },
        }),
      );
    vi.stubGlobal("fetch", fetchMock);

    await expect(loadQuranData()).rejects.toThrow("returned 503");
    expect((await loadQuranData()).surahs).toHaveLength(114);
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });
});
