import { afterEach, describe, expect, it, vi } from "vite-plus/test";

vi.mock("$lib/config/site", () => ({
  QURAN: { apiBase: "https://api.test/quran" },
}));

import { quranApi } from "$lib/quran/api-client";

function okJson(data: unknown): Response {
  return new Response(JSON.stringify(data), {
    status: 200,
    headers: { "content-type": "application/json" },
  });
}

const UTHMANI_SURAH_1 = {
  data: {
    sourceId: "uthmani",
    script: "uthmani",
    verses: ["ٱلْحَمْدُ", "لِلَّهِ"],
    normalization: {
      surah: 1,
      sourceId: "uthmani",
      script: "uthmani",
      sourceProfile: "tanzil-uthmani-581cc540",
      packaging: "numbered-ayah",
      openerKind: "verse",
      openerText: "ٱلْحَمْدُ",
      openerEndScalar: 0,
      bodyStartScalar: 0,
    },
  },
};

describe("quranApi", () => {
  afterEach(() => vi.restoreAllMocks());

  it("readSurah hits /sources/{id}/surah/{n}, unwraps the envelope, decodes", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(okJson(UTHMANI_SURAH_1));
    const surah = await quranApi.readSurah("uthmani", 1);
    expect(fetchMock).toHaveBeenCalledWith(
      "https://api.test/quran/sources/uthmani/surah/1",
      expect.objectContaining({ headers: { accept: "application/json" } }),
    );
    expect(surah.sourceId).toBe("uthmani");
    expect(surah.verses).toEqual(["ٱلْحَمْدُ", "لِلَّهِ"]);
    expect(surah.normalization.packaging).toBe("numbered-ayah");
  });

  it("readSurah rejects a malformed payload", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(okJson({ data: { sourceId: "uthmani" } }));
    await expect(quranApi.readSurah("uthmani", 1)).rejects.toThrow(/malformed surah/);
  });

  it("search decodes results and tags the source as api", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      okJson({
        data: {
          query: "الحمد",
          total: 1,
          limit: 20,
          offset: 0,
          results: [
            {
              kind: "ayah",
              ayah: { key: "1:2", surah: 1, ayah: 2, globalIndex: 2, text: "الحمد" },
              highlights: [{ start: 0, end: 5 }],
            },
          ],
        },
      }),
    );
    const res = await quranApi.search("الحمد");
    expect(res.source).toBe("api");
    expect(res.total).toBe(1);
    const first = res.results[0];
    expect(first?.kind).toBe("ayah");
    if (first?.kind === "ayah") expect(first.ayah.key).toBe("1:2");
  });

  it("search throws on a non-ok response", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response("nope", { status: 500 }));
    await expect(quranApi.search("الحمد")).rejects.toThrow(/quran api 500/);
  });
});
