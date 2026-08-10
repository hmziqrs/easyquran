import { afterAll, beforeAll, beforeEach, describe, expect, it, vi } from "vite-plus/test";

vi.mock("$lib/config/site", () => ({ QURAN: { apiBase: "https://api.test/quran" } }));

const { track, consentState } = vi.hoisted(() => ({
  track: vi.fn(),
  consentState: { analytics: true },
}));
vi.mock("$lib/firebase/analytics", () => ({ track }));
vi.mock("$lib/stores/consent.svelte", () => ({ consent: consentState }));

import { OpenerKind, OpenerPackaging, QuranScript, QuranSourceId } from "$lib/data/quran-types";
import { RangeKind } from "$lib/data/quran-data";
import { SearchHitKind, type SearchHit } from "$lib/quran/search/types";
import {
  buildArabicBakedMap,
  decodeQuranRangeText,
  decodeQuranSurahText,
  decodeScript,
  decodeScriptsPayload,
  decodeSearchHit,
  decodeSearchResponse,
  decodeSourcesPayload,
  decodeTranslationRangeText,
  decodeTranslationSurahText,
  parseArtifactUrl,
  reportArtifactRejection,
  unwrapEnvelope,
  validateArtifactAgainstBaked,
  type BakedArtifactEntry,
} from "$lib/quran/wire";
import { QURAN_DATA } from "$lib/server/quran-data";
import {
  loadTranslationRangeData,
  loadTranslationSurahRouteData,
  type TranslationFetcher,
} from "$lib/server/quran-translation-page";

const validateCoordinate = (globalIndex: number, surah: number, ayah: number): boolean =>
  QURAN_DATA.globalIndexOf(surah, ayah) === globalIndex;

const AYAH_HIT: SearchHit = {
  kind: SearchHitKind.Ayah,
  ayah: {
    key: "2:255",
    surah: 2,
    ayah: 255,
    globalIndex: 262,
    text: "اللَّهُ لاَ إِلَـٰهَ إِلاَّ هُوَ",
  },
  highlights: [{ start: 0, end: 7 }],
};

const OPENER_HIT: SearchHit = {
  kind: SearchHitKind.Opener,
  key: "opener:2",
  surah: 2,
  anchorAyah: 1,
  text: "بِسْمِ ٱللَّهِ",
  highlights: [{ start: 0, end: 14 }],
};

describe("unwrapEnvelope", () => {
  it("unwraps data but preserves nullish and non-object bodies", () => {
    expect(unwrapEnvelope({ data: { results: [] } })).toEqual({ results: [] });
    const nullish = { data: null, extra: 1 };
    expect(unwrapEnvelope(nullish)).toBe(nullish);
    expect(unwrapEnvelope("body")).toBe("body");
  });
});

describe("canonical search wire", () => {
  it.each([AYAH_HIT, OPENER_HIT])("rebuilds a tagged hit", (value) => {
    const decoded = decodeSearchHit(value, validateCoordinate);
    expect(decoded).toEqual(value);
    expect(decoded).not.toBe(value);
  });

  it("rejects legacy ayah-only and malformed tagged shapes", () => {
    expect(decodeSearchHit({ surah: 2, ayah: 1, highlights: [] }, validateCoordinate)).toBeNull();
    expect(
      decodeSearchHit({ ...AYAH_HIT, ayah: { ...AYAH_HIT.ayah, surah: "2" } }, validateCoordinate),
    ).toBeNull();
    expect(
      decodeSearchHit(
        { ...AYAH_HIT, ayah: { ...AYAH_HIT.ayah, globalIndex: 263 } },
        validateCoordinate,
      ),
    ).toBeNull();
    expect(
      decodeSearchHit({ ...AYAH_HIT, highlights: [{ start: 4, end: 2 }] }, validateCoordinate),
    ).toBeNull();
    expect(decodeSearchHit({ ...OPENER_HIT, anchorAyah: 2 }, validateCoordinate)).toBeNull();
  });

  it("decodes a response and fails closed on malformed individual hits", () => {
    expect(
      decodeSearchResponse(
        {
          query: "الله",
          total: 2,
          limit: 20,
          offset: 0,
          results: [AYAH_HIT, OPENER_HIT],
        },
        validateCoordinate,
      ),
    ).toEqual({
      query: "الله",
      total: 2,
      limit: 20,
      offset: 0,
      results: [AYAH_HIT, OPENER_HIT],
    });
    expect(
      decodeSearchResponse(
        {
          query: "الله",
          total: 2,
          limit: 20,
          offset: 0,
          results: [AYAH_HIT, { ...OPENER_HIT, anchorAyah: 2 }, OPENER_HIT],
        },
        validateCoordinate,
      ),
    ).toBeNull();
    expect(decodeSearchResponse({ results: "invalid" })).toBeNull();
  });
});

describe("normalized surah Worker wire", () => {
  const payload = {
    sourceId: QuranSourceId.TanzilUthmani,
    script: QuranScript.Uthmani,
    verses: ["raw first", "raw second"],
    normalization: {
      surah: 2,
      sourceId: QuranSourceId.TanzilUthmani,
      script: QuranScript.Uthmani,
      sourceProfile: "tanzil-uthmani-581cc540",
      packaging: OpenerPackaging.EmbeddedPrefix,
      openerKind: OpenerKind.Header,
      openerText: "opener",
      openerEndScalar: 6,
      bodyStartScalar: 7,
    },
  };

  it("rebuilds raw verses and their descriptor", () => {
    expect(decodeQuranSurahText(payload)).toEqual(payload);
  });

  it("rejects a mismatched source identity, script, or scalar cut", () => {
    expect(
      decodeQuranSurahText({
        ...payload,
        normalization: { ...payload.normalization, sourceProfile: "unregistered" },
      }),
    ).toBeNull();
    expect(
      decodeQuranSurahText({
        ...payload,
        normalization: { ...payload.normalization, script: QuranScript.SimpleClean },
      }),
    ).toBeNull();
    expect(
      decodeQuranSurahText({
        ...payload,
        normalization: { ...payload.normalization, openerEndScalar: 8 },
      }),
    ).toBeNull();
  });
});

describe("coordinate-aware range Worker wire", () => {
  const normalization = {
    surah: 2,
    sourceId: QuranSourceId.TanzilUthmani,
    script: QuranScript.Uthmani,
    sourceProfile: "tanzil-uthmani-581cc540",
    packaging: OpenerPackaging.EmbeddedPrefix,
    openerKind: OpenerKind.Header,
    openerText: "opener",
    openerEndScalar: 6,
    bodyStartScalar: 7,
  } as const;
  const payload = {
    ayahs: [
      { key: "2:1", surah: 2, ayah: 1, globalIndex: 8, text: "first" },
      { key: "2:2", surah: 2, ayah: 2, globalIndex: 9, text: "second" },
    ],
    normalizations: [normalization],
  };

  it("preserves explicit coordinates for a clipped page", () => {
    expect(decodeQuranRangeText(payload, validateCoordinate)).toEqual(payload);
  });

  it("rejects gaps, coordinate mismatches, and missing normalization", () => {
    expect(
      decodeQuranRangeText(
        {
          ...payload,
          ayahs: [payload.ayahs[0], { ...payload.ayahs[1], globalIndex: 10 }],
        },
        validateCoordinate,
      ),
    ).toBeNull();
    expect(
      decodeQuranRangeText(
        { ...payload, ayahs: [{ ...payload.ayahs[0], key: "2:2" }] },
        validateCoordinate,
      ),
    ).toBeNull();
    expect(decodeQuranRangeText({ ...payload, normalizations: [] })).toBeNull();
  });
});

describe("manifest wire", () => {
  const scripts = [
    { id: QuranSourceId.TanzilUthmani, sizeBytes: 1, downloadUrl: "https://x/u" },
    { id: QuranSourceId.TanzilSimpleClean, sizeBytes: 2, downloadUrl: "https://x/s" },
  ];

  it("decodes registered script ids and script envelopes", () => {
    expect(decodeScript(scripts[0])).toEqual(scripts[0]);
    expect(decodeScript({ ...scripts[0], id: "unknown" })).toBeNull();
    expect(decodeScript({ ...scripts[0], sizeBytes: 0 })).toBeNull();
    expect(decodeScript({ ...scripts[0], sizeBytes: -1 })).toBeNull();
    expect(decodeScript({ ...scripts[0], sizeBytes: "1" })).toBeNull();
    expect(decodeScriptsPayload({ data: { scripts } })).toEqual(scripts);
  });
});

describe("translation surah Worker wire", () => {
  const payload = {
    sourceId: "en.sahih",
    script: QuranScript.Translation,
    verses: ["first", "second"],
    normalization: {
      surah: 2,
      sourceId: "en.sahih",
      script: QuranScript.Translation,
      sourceProfile: "en.sahih",
      packaging: OpenerPackaging.Absent,
      openerKind: OpenerKind.None,
      openerText: null,
      openerEndScalar: 0,
      bodyStartScalar: 0,
    },
  };

  it("rebuilds translation verses and their descriptor", () => {
    expect(decodeTranslationSurahText(payload)).toEqual(payload);
  });

  it("rejects arabic script, non-zero scalars, and any opener-shaped descriptor", () => {
    expect(decodeTranslationSurahText({ ...payload, script: QuranScript.Uthmani })).toBeNull();
    expect(
      decodeTranslationSurahText({
        ...payload,
        normalization: { ...payload.normalization, script: QuranScript.Uthmani },
      }),
    ).toBeNull();
    expect(
      decodeTranslationSurahText({
        ...payload,
        normalization: { ...payload.normalization, openerEndScalar: 1 },
      }),
    ).toBeNull();
    expect(
      decodeTranslationSurahText({
        ...payload,
        normalization: { ...payload.normalization, bodyStartScalar: 1 },
      }),
    ).toBeNull();
    expect(
      decodeTranslationSurahText({
        ...payload,
        normalization: { ...payload.normalization, packaging: OpenerPackaging.EmbeddedPrefix },
      }),
    ).toBeNull();
    expect(
      decodeTranslationSurahText({
        ...payload,
        normalization: { ...payload.normalization, openerKind: OpenerKind.Header },
      }),
    ).toBeNull();
    expect(
      decodeTranslationSurahText({
        ...payload,
        normalization: { ...payload.normalization, openerText: "opener" },
      }),
    ).toBeNull();
    expect(
      decodeTranslationSurahText({
        ...payload,
        normalization: { ...payload.normalization, sourceId: "en.other" },
      }),
    ).toBeNull();
    expect(decodeTranslationSurahText({ ...payload, verses: ["ok", 3] })).toBeNull();
  });
});

describe("translation range Worker wire", () => {
  const normalization = {
    surah: 2,
    sourceId: "en.sahih",
    script: QuranScript.Translation,
    sourceProfile: "en.sahih",
    packaging: OpenerPackaging.Absent,
    openerKind: OpenerKind.None,
    openerText: null,
    openerEndScalar: 0,
    bodyStartScalar: 0,
  };
  const payload = {
    ayahs: [
      { key: "2:1", surah: 2, ayah: 1, globalIndex: 8, text: "first" },
      { key: "2:2", surah: 2, ayah: 2, globalIndex: 9, text: "second" },
    ],
    normalizations: [normalization],
  };

  it("preserves translation coordinates for a clipped page", () => {
    expect(decodeTranslationRangeText(payload)).toEqual(payload);
  });

  it("rejects gaps, missing normalization, and non-translation descriptors", () => {
    expect(
      decodeTranslationRangeText({
        ...payload,
        ayahs: [payload.ayahs[0], { ...payload.ayahs[1], globalIndex: 10 }],
      }),
    ).toBeNull();
    expect(decodeTranslationRangeText({ ...payload, normalizations: [] })).toBeNull();
    expect(
      decodeTranslationRangeText({
        ...payload,
        normalizations: [{ ...normalization, script: QuranScript.Uthmani }],
      }),
    ).toBeNull();
    expect(
      decodeTranslationRangeText({
        ...payload,
        normalizations: [{ ...normalization, openerEndScalar: 1 }],
      }),
    ).toBeNull();
  });
});

describe("translation sources manifest wire", () => {
  const arabic = {
    id: QuranSourceId.TanzilUthmani,
    sizeBytes: 1,
    downloadUrl: "https://x/u",
  };
  const translation = {
    id: "en.sahih",
    language: "English",
    languageCode: "en",
    direction: "ltr",
    name: "Sahih International",
    translator: null,
    sizeBytes: 2,
    downloadUrl: "https://x/t",
  };

  it("decodes mixed arabic and translation entries from scripts or sources envelopes", () => {
    const expected = [
      { kind: "arabic", spec: arabic },
      { kind: "translation", entry: translation },
    ];
    expect(
      decodeSourcesPayload({
        data: { scripts: [arabic, { ...translation, kind: "translation" }] },
      }),
    ).toEqual(expected);
    expect(
      decodeSourcesPayload({ data: { sources: [{ ...arabic, kind: "arabic" }, translation] } }),
    ).toEqual(expected);
  });

  it("fails closed on a malformed entry or a kind that does not match its body", () => {
    expect(
      decodeSourcesPayload({ data: { scripts: [arabic, { ...translation, sizeBytes: 0 }] } }),
    ).toBeNull();
    expect(
      decodeSourcesPayload({ data: { scripts: [{ ...arabic, kind: "translation" }] } }),
    ).toBeNull();
    expect(
      decodeSourcesPayload({ data: { scripts: [{ ...translation, kind: "arabic" }] } }),
    ).toBeNull();
    expect(decodeSourcesPayload({ data: { other: [] } })).toBeNull();
  });
});

describe("translation route loaders", () => {
  const SOURCES_PAYLOAD = {
    data: {
      sources: [
        {
          kind: "translation",
          id: "en.sahih",
          language: "English",
          languageCode: "en",
          direction: "ltr",
          name: "Saheeh International",
          translator: "Saheeh International",
          sizeBytes: 1048576,
          downloadUrl: "https://cdn.test/en.sahih.bin",
        },
      ],
    },
  };

  beforeAll(() => {
    vi.stubGlobal("fetch", async (input: RequestInfo | URL) => {
      const url =
        input instanceof URL ? input.toString() : typeof input === "string" ? input : input.url;
      return url.endsWith("/sources")
        ? new Response(JSON.stringify(SOURCES_PAYLOAD), {
            status: 200,
            headers: { "content-type": "application/json" },
          })
        : new Response("", { status: 404 });
    });
  });

  afterAll(() => {
    vi.unstubAllGlobals();
  });

  type RangeAyah = { key: string; surah: number; ayah: number; globalIndex: number; text: string };
  type RangePayload = { ayahs: RangeAyah[]; normalizations: unknown[] };

  function translationRangeFor(from: number, to: number, sourceId = "en.sahih"): RangePayload {
    const ayahs: RangeAyah[] = [];
    const surahNums = new Set<number>();
    for (let g = from; g <= to; g++) {
      const key = QURAN_DATA.verseKeyAtGlobal(g);
      if (!key) throw new Error(`no verse at global ${g}`);
      const [surah, ayah] = key.split(":").map(Number);
      surahNums.add(surah);
      ayahs.push({ key, surah, ayah, globalIndex: g, text: `v-${g}` });
    }
    const normalizations = [...surahNums].map((surah) => ({
      surah,
      sourceId,
      script: QuranScript.Translation,
      sourceProfile: sourceId,
      packaging: OpenerPackaging.Absent,
      openerKind: OpenerKind.None,
      openerText: null,
      openerEndScalar: 0,
      bodyStartScalar: 0,
    }));
    return { ayahs, normalizations };
  }

  function fetcherReturning(body: unknown, status = 200): TranslationFetcher {
    return async () =>
      new Response(JSON.stringify(body), {
        status,
        headers: { "content-type": "application/json" },
      });
  }

  it("loads a surah page, decodes it, and drops previousPage on localPage 1", async () => {
    const surah = QURAN_DATA.surahByNum(1)!;
    const page = QURAN_DATA.surahLocalPage(1, 1)!;
    const data = await loadTranslationSurahRouteData(
      surah,
      1,
      "en",
      "sahih",
      fetcherReturning(translationRangeFor(page.startGlobal, page.endGlobal)),
    );
    expect(data).toBeDefined();
    expect(data!.pageData.ayahs).toHaveLength(page.endGlobal - page.startGlobal + 1);
    expect(data!.previousPage).toBeNull();
  });

  it("links the previous page when localPage is greater than 1", async () => {
    const surah = QURAN_DATA.surahByNum(2)!;
    const page = QURAN_DATA.surahLocalPage(2, 2)!;
    const data = await loadTranslationSurahRouteData(
      surah,
      2,
      "en",
      "sahih",
      fetcherReturning(translationRangeFor(page.startGlobal, page.endGlobal)),
    );
    expect(data!.previousPage).not.toBeNull();
    expect(data!.previousPage!.localPage).toBe(1);
  });

  it("returns undefined when the surah local page does not exist", async () => {
    const surah = QURAN_DATA.surahByNum(1)!;
    const overflow = QURAN_DATA.surahLocalPageCount(1) + 1;
    await expect(
      loadTranslationSurahRouteData(surah, overflow, "en", "sahih", fetcherReturning({})),
    ).resolves.toBeUndefined();
  });

  it("throws when the range does not cover the surah contiguously", async () => {
    const surah = QURAN_DATA.surahByNum(1)!;
    const page = QURAN_DATA.surahLocalPage(1, 1)!;
    const overflowing = translationRangeFor(page.startGlobal, page.startGlobal + surah.ayahCount);
    await expect(
      loadTranslationSurahRouteData(surah, 1, "en", "sahih", fetcherReturning(overflowing)),
    ).rejects.toThrow(/contiguously/);
  });

  it("loads a global page range and surfaces its ayahs", async () => {
    const entry = QURAN_DATA.rangeByIndex(RangeKind.Page, 1)!;
    const data = await loadTranslationRangeData(
      "page",
      1,
      "en",
      "sahih",
      fetcherReturning(translationRangeFor(entry.startGlobal, entry.endGlobal)),
    );
    expect(data.kind).toBe("page");
    expect(data.index).toBe(1);
    expect(data.ayahs).toHaveLength(entry.endGlobal - entry.startGlobal + 1);
  });

  it("rejects an unknown range index with a 404", async () => {
    const overflow = QURAN_DATA.rangeCount(RangeKind.Page) + 1;
    await expect(
      loadTranslationRangeData("page", overflow, "en", "sahih", fetcherReturning({})),
    ).rejects.toMatchObject({ status: 404 });
  });
});

describe("baked artifact delivery contract", () => {
  const baked: BakedArtifactEntry = {
    sizeBytes: 100,
    r2Path: "/tanzil/arabic/quran-uthmani.sqlite",
    sameOriginDeliveryPath: "/_quran/tanzil/arabic/quran-uthmani.sqlite",
  };

  describe("buildArabicBakedMap", () => {
    it("strips the artifactBase prefix to derive the canonical R2 path", () => {
      const map = buildArabicBakedMap(
        [
          {
            id: "uthmani",
            sizeBytes: 100,
            downloadUrl: "/_quran/tanzil/arabic/quran-uthmani.sqlite",
          },
        ],
        "/_quran",
      );
      expect(map.get("uthmani")).toEqual({
        sizeBytes: 100,
        r2Path: "/tanzil/arabic/quran-uthmani.sqlite",
        sameOriginDeliveryPath: "/_quran/tanzil/arabic/quran-uthmani.sqlite",
      });
    });
  });

  describe("parseArtifactUrl", () => {
    it("accepts a clean https url whose pathname equals the baked R2 path", () => {
      expect(
        parseArtifactUrl("https://r2.easyquran.fyi/tanzil/arabic/quran-uthmani.sqlite", baked),
      ).toBeInstanceOf(URL);
    });

    it.each([
      ["http origin", "http://r2.easyquran.fyi/tanzil/arabic/quran-uthmani.sqlite"],
      ["credentials", "https://user:pass@r2.easyquran.fyi/tanzil/arabic/quran-uthmani.sqlite"],
      ["query string", "https://r2.easyquran.fyi/tanzil/arabic/quran-uthmani.sqlite?x=1"],
      ["fragment", "https://r2.easyquran.fyi/tanzil/arabic/quran-uthmani.sqlite#sec"],
      ["canonical path mismatch", "https://r2.easyquran.fyi/evil/quran-uthmani.sqlite"],
      ["non-https scheme", "ftp://r2.easyquran.fyi/tanzil/arabic/quran-uthmani.sqlite"],
    ])("rejects %s", (_label, url) => {
      expect(parseArtifactUrl(url, baked)).toBeNull();
    });

    it.each([
      ["empty string", ""],
      ["non-string", 42],
      ["unparseable", "not-a-url"],
    ])("rejects %s", (_label, value) => {
      expect(parseArtifactUrl(value, baked)).toBeNull();
    });
  });

  describe("validateArtifactAgainstBaked", () => {
    const map = new Map([["uthmani", baked]]);

    it("reconstructs size and download url from baked fields on a valid entry", () => {
      expect(
        validateArtifactAgainstBaked(
          "uthmani",
          100,
          "https://r2.easyquran.fyi/tanzil/arabic/quran-uthmani.sqlite",
          map,
        ),
      ).toEqual({
        id: "uthmani",
        sizeBytes: 100,
        downloadUrl: "/_quran/tanzil/arabic/quran-uthmani.sqlite",
      });
    });

    it.each([
      [
        "unknown id",
        ["missing", 100, "https://r2.easyquran.fyi/tanzil/arabic/quran-uthmani.sqlite"],
      ],
      [
        "size mismatch",
        ["uthmani", 101, "https://r2.easyquran.fyi/tanzil/arabic/quran-uthmani.sqlite"],
      ],
      ["path mismatch", ["uthmani", 100, "https://r2.easyquran.fyi/evil/quran-uthmani.sqlite"]],
    ])("rejects %s", (_label, [id, sizeBytes, downloadUrl]) => {
      expect(
        validateArtifactAgainstBaked(id as string, sizeBytes as number, downloadUrl as string, map),
      ).toBeNull();
    });
  });
});

describe("artifact rejection telemetry", () => {
  beforeEach(() => {
    track.mockReset();
    consentState.analytics = true;
  });

  it("emits a structured event through the consent-gated channel", async () => {
    reportArtifactRejection("scripts_payload");
    await vi.waitFor(() => {
      expect(track).toHaveBeenCalledWith("quran_artifact_rejected", { reason: "scripts_payload" });
    });
  });

  it("suppresses the event when analytics consent is denied", async () => {
    consentState.analytics = false;
    reportArtifactRejection("scripts_payload");
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(track).not.toHaveBeenCalled();
  });
});
