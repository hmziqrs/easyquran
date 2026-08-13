import { SearchProvider } from "$lib/quran/search/types";
import { afterEach, describe, expect, it, vi } from "vite-plus/test";

vi.mock("$lib/config/site", () => ({ QURAN: { apiBase: "https://api.test/quran" } }));

const forwarded = vi.fn();
const searchMock = vi.fn().mockResolvedValue({
  query: "x",
  total: 0,
  limit: 20,
  offset: 0,
  results: [],
  source: SearchProvider.Api,
});

vi.mock("$lib/quran/api-client", () => ({
  quranApi: { search: (...a: unknown[]) => searchMock(...a) },
}));

vi.mock("$lib/quran/worker-client", () => ({
  quranWorker: { ready: false, search: vi.fn() },
}));

vi.mock("$lib/data/quran-data-client", () => ({
  loadQuranData: async () => ({
    surahs: [{ num: 1, name: "Fatiha", arabic: "الفاتحة", startGlobal: 0 }],
    globalIndexOf: (surah: number, ayah: number) => {
      forwarded();
      // surah 1 starts at global 0, so ayah N -> global N-1
      return surah === 1 ? ayah - 1 : -1;
    },
  }),
}));

import { quranSearch } from "$lib/quran/search";

describe("quranSearch api fallback", () => {
  afterEach(() => vi.clearAllMocks());

  it("forwards the coordinate validator to quranApi.search when the worker is not ready", async () => {
    await quranSearch("x");
    expect(searchMock).toHaveBeenCalledTimes(1);
    const args = searchMock.mock.calls[0]!;
    // search(query, opts, signal, validateCoordinate)
    const validate = args[3];
    expect(typeof validate).toBe("function");
    // validator must actually be wired to the catalog
    const validator = validate as (g: number, s: number, a: number) => boolean;
    expect(validator(0, 1, 1)).toBe(true); // global 0 = surah 1 ayah 1
    expect(validator(99, 1, 1)).toBe(false);
    expect(forwarded).toHaveBeenCalled();
  });
});
