import { OpenerKind, OpenerPackaging, QuranScript } from "$lib/data/quran-types";
import type { Ayah, QuranRangeText, SurahNormalization } from "$lib/data/quran-types";
import { afterEach, describe, expect, it, vi } from "vite-plus/test";

vi.mock("$lib/config/site", () => ({
  QURAN: { apiBase: "https://api.test/quran" },
}));

import { quranApi } from "$lib/quran/api-client";
import { FetchHttpError, MalformedDataError, RESPONSE_CAP } from "$lib/quran/fetch";

type ApiEnvelopeBody = { data: object };

function okJson(body: ApiEnvelopeBody): Response {
  return new Response(JSON.stringify(body), {
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

  it("search throws a FetchHttpError without leaking the URL on a non-ok response", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response("nope", { status: 500 }));
    await expect(quranApi.search("الحمد")).rejects.toThrow(/http 500/);
    await expect(quranApi.search("الحمد")).rejects.toBeInstanceOf(FetchHttpError);
  });

  it("search threads the coordinate validator and rejects when it disagrees", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      okJson({
        data: {
          query: "x",
          total: 1,
          limit: 20,
          offset: 0,
          results: [
            {
              kind: "ayah",
              ayah: { key: "1:2", surah: 1, ayah: 2, globalIndex: 2, text: "x" },
              highlights: [{ start: 0, end: 1 }],
            },
          ],
        },
      }),
    );
    const rejectAll = (): boolean => false;
    await expect(quranApi.search("x", {}, undefined, rejectAll)).rejects.toBeInstanceOf(
      MalformedDataError,
    );
  });
});

function translationNorm(surah: number): SurahNormalization {
  return {
    surah,
    sourceId: "en.x",
    script: QuranScript.Translation,
    sourceProfile: "translation-profile",
    packaging: OpenerPackaging.Absent,
    openerKind: OpenerKind.None,
    openerText: null,
    openerEndScalar: 0,
    bodyStartScalar: 0,
  };
}

function translationRange(from: number, to: number): QuranRangeText {
  const ayahs: Ayah[] = [];
  const bySurah = new Map<number, SurahNormalization>();
  for (let g = from; g <= to; g++) {
    const surah = Math.min(114, Math.ceil(g / 7));
    const ayah = g - (surah - 1) * 7;
    ayahs.push({ key: `${surah}:${ayah}`, surah, ayah, globalIndex: g, text: `t${g}` });
    bySurah.set(surah, translationNorm(surah));
  }
  return { ayahs, normalizations: [...bySurah.values()] };
}

describe("quranApi.readRange", () => {
  afterEach(() => vi.restoreAllMocks());

  it("issues a single request for an in-cap range and stitches the result", async () => {
    const fetchMock = vi
      .spyOn(globalThis, "fetch")
      .mockResolvedValue(okJson({ data: translationRange(1, 10) }));
    const range = await quranApi.readRange("en.x", 1, 10);
    expect(range.ayahs.map((a) => a.globalIndex)).toEqual([1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it("chunks an oversized range into two cap-bounded requests and stitches", async () => {
    const from = 1;
    const to = RESPONSE_CAP + 5;
    const mid = from + RESPONSE_CAP - 1;
    const fetchMock = vi
      .spyOn(globalThis, "fetch")
      .mockResolvedValueOnce(okJson({ data: translationRange(from, mid) }))
      .mockResolvedValueOnce(okJson({ data: translationRange(mid + 1, to) }));
    const range = await quranApi.readRange("en.x", from, to);
    expect(fetchMock).toHaveBeenCalledTimes(2);
    // SAFETY: quranApi.readRange passes a template-string URL to fetch, so each spy call's first arg is a string
    const firstUrl = fetchMock.mock.calls[0]![0] as string;
    // SAFETY: same string-URL contract as the first chunk call above
    const secondUrl = fetchMock.mock.calls[1]![0] as string;
    expect(firstUrl).toContain(`from=${from}&to=${mid}`);
    expect(secondUrl).toContain(`from=${mid + 1}&to=${to}`);
    expect(range.ayahs).toHaveLength(RESPONSE_CAP + 5);
    expect(range.ayahs[0]!.globalIndex).toBe(from);
    expect(range.ayahs.at(-1)!.globalIndex).toBe(to);
  });

  it("threads the coordinate validator through the decode path", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(okJson({ data: translationRange(1, 5) }));
    const rejectAll = (): boolean => false;
    await expect(quranApi.readRange("en.x", 1, 5, undefined, rejectAll)).rejects.toBeInstanceOf(
      MalformedDataError,
    );
  });

  it("rejects with FetchHttpError when the api returns a non-ok status", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response("nope", { status: 504 }));
    await expect(quranApi.readRange("en.x", 1, 5)).rejects.toBeInstanceOf(FetchHttpError);
  });
});
