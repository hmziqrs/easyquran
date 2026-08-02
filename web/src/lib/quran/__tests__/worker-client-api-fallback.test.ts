import { describe, expect, it, vi } from "vite-plus/test";

vi.mock("$lib/config/site", () => ({
  QURAN: { apiBase: "https://api.test/quran" },
}));

import { quranWorker } from "$lib/quran/worker-client";

const SURAH1 = {
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

describe("quranWorker.readSurah API fallback", () => {
  it("serves from the live API when the wasm worker is not started (no-download mode)", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify(SURAH1), {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
    );
    const surah = await quranWorker.readSurah(1);
    expect(fetchMock).toHaveBeenCalledWith(
      "https://api.test/quran/sources/uthmani/surah/1",
      expect.objectContaining({ headers: { accept: "application/json" } }),
    );
    expect(surah.sourceId).toBe("uthmani");
    expect(surah.verses).toEqual(["ٱلْحَمْدُ", "لِلَّهِ"]);
  });
});
