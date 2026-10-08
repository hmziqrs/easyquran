import { describe, expect, it } from "vite-plus/test";

import {
  HIZB_COUNT,
  hizbOfPage,
  hizbRange,
  juzOfPage,
  pageOfGlobal,
  positionForGlobal,
  quarterOfPage,
  RUB_COUNT,
} from "#lib/data/mushaf-divisions.js";
import { RANGE_COUNTS, RangeKind } from "#lib/data/quran-data.js";
import { QURAN_DATA } from "#lib/server/quran-data.js";

describe("mushaf divisions from the baked JSON", () => {
  it("derives 60 hizbs from 240 quarters with no remainder", () => {
    expect(RANGE_COUNTS[RangeKind.HizbQuarter]).toBe(240);
    expect(RUB_COUNT).toBe(240);
    expect(HIZB_COUNT).toBe(60);
    expect(RUB_COUNT % 4).toBe(0);
  });

  it("hizb 1 starts at 1:1 and hizb 60 ends at the end of the corpus", () => {
    const first = hizbRange(QURAN_DATA, 1);
    expect(first?.first).toBe("1:1");
    expect(first?.startGlobal).toBe(1);
    const last = hizbRange(QURAN_DATA, HIZB_COUNT);
    expect(last?.last).toBe("114:6");
    expect(last?.endGlobal).toBe(6236);
  });

  it("hizb i spans quarters 4i-3..4i and each hizb keeps its quarters' extents", () => {
    for (let hizb = 1; hizb <= HIZB_COUNT; hizb += 1) {
      const range = hizbRange(QURAN_DATA, hizb);
      const firstQuarter = QURAN_DATA.rangeByIndex(RangeKind.HizbQuarter, hizb * 4 - 3);
      const lastQuarter = QURAN_DATA.rangeByIndex(RangeKind.HizbQuarter, hizb * 4);
      expect(range).toBeDefined();
      expect(firstQuarter).toBeDefined();
      expect(lastQuarter).toBeDefined();
      expect(range?.startGlobal).toBe(firstQuarter?.startGlobal);
      expect(range?.endGlobal).toBe(lastQuarter?.endGlobal);
      expect(range?.index).toBe(hizb);
    }
  });

  it("hizb ranges tile 1..6236 with no gap and no overlap", () => {
    let expected = 1;
    for (let hizb = 1; hizb <= HIZB_COUNT; hizb += 1) {
      const range = hizbRange(QURAN_DATA, hizb);
      expect(range?.startGlobal).toBe(expected);
      expected = range!.endGlobal + 1;
    }
    expect(expected).toBe(6237);
  });

  it("maps a global ayah to its page, juz, quarter, and hizb", () => {
    expect(pageOfGlobal(QURAN_DATA, 1)).toBe(1);
    expect(juzOfPage(QURAN_DATA, 1)).toBe(1);
    expect(quarterOfPage(QURAN_DATA, 1)).toBe(1);
    expect(hizbOfPage(QURAN_DATA, 1)).toBe(1);
    // The last ayah of the mushaf sits in page 604 / juz 30 / hizb 60.
    expect(pageOfGlobal(QURAN_DATA, 6236)).toBe(604);
    expect(juzOfPage(QURAN_DATA, 6236)).toBe(30);
    expect(hizbOfPage(QURAN_DATA, 6236)).toBe(60);
  });

  it("hizbOfPage is ceil(quarter / 4) at every quarter boundary", () => {
    for (let quarter = 1; quarter <= RUB_COUNT; quarter += 1) {
      const entry = QURAN_DATA.rangeByIndex(RangeKind.HizbQuarter, quarter);
      if (!entry) throw new Error(`missing quarter ${quarter}`);
      expect(hizbOfPage(QURAN_DATA, entry.startGlobal)).toBe(Math.ceil(quarter / 4));
    }
  });

  it("rejects out-of-range hizb requests", () => {
    expect(hizbRange(QURAN_DATA, 0)).toBeUndefined();
    expect(hizbRange(QURAN_DATA, HIZB_COUNT + 1)).toBeUndefined();
  });

  it("derives the sticky-position triplet from one global ayah (page 2 payload spot-check)", () => {
    // Mushaf page 2 opens in juz 1, hizb 1.
    const page2 = QURAN_DATA.rangeByIndex(RangeKind.Page, 2);
    if (!page2) throw new Error("missing mushaf page 2");
    expect(positionForGlobal(QURAN_DATA, page2.startGlobal)).toEqual({
      globalPage: 2,
      juz: 1,
      hizb: 1,
    });
  });
});
