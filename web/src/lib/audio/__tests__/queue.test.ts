import { describe, expect, it } from "vite-plus/test";

import {
  MAX_QUEUE_ITEMS,
  buildQueue,
  currentItem,
  queueAt,
  stepQueue,
  surahBoundary,
} from "../queue";

describe("buildQueue", () => {
  it("flattens segments in play order", () => {
    const queue = buildQueue([
      { surah: 1, from: 1, to: 2 },
      { surah: 2, from: 1, to: 1 },
    ]);
    expect(queue?.items).toEqual([
      { surah: 1, ayah: 1 },
      { surah: 1, ayah: 2 },
      { surah: 2, ayah: 1 },
    ]);
    expect(queue?.index).toBe(0);
  });

  it("returns null for no segments or an invalid segment", () => {
    expect(buildQueue([])).toBeNull();
    expect(buildQueue([{ surah: 0, from: 1, to: 1 }])).toBeNull();
    expect(buildQueue([{ surah: 1, from: 3, to: 2 }])).toBeNull();
    expect(buildQueue([{ surah: 1, from: 0, to: 2 }])).toBeNull();
  });

  it("refuses a queue over the item cap", () => {
    expect(buildQueue([{ surah: 2, from: 1, to: MAX_QUEUE_ITEMS + 1 }])).toBeNull();
  });
});

describe("stepQueue and queueAt", () => {
  const queue = buildQueue([{ surah: 1, from: 1, to: 3 }]);

  it("moves one ayah and stops at both ends", () => {
    expect(queue && stepQueue(queue, 1)?.index).toBe(1);
    expect(queue && stepQueue(queue, -1)).toBeNull();
    const last = queue && queueAt(queue, 2);
    expect(last && stepQueue(last, 1)).toBeNull();
  });

  it("rejects non-integer and out-of-range indexes", () => {
    expect(queue && queueAt(queue, 1.5)).toBeNull();
    expect(queue && queueAt(queue, 3)).toBeNull();
  });

  it("reads the current item", () => {
    expect(queue && currentItem(queue)).toEqual({ surah: 1, ayah: 1 });
  });
});

describe("surahBoundary", () => {
  const queue = buildQueue([
    { surah: 1, from: 1, to: 2 },
    { surah: 2, from: 1, to: 2 },
    { surah: 3, from: 1, to: 1 },
  ]);

  it("finds the first ayah of the next surah", () => {
    // No surah before the first ayah: the previous-surah boundary is null.
    expect(queue && surahBoundary(queue, -1)).toBeNull();
    const inFirst = queue && queueAt(queue, 0);
    expect(inFirst && surahBoundary(inFirst, 1)).toBe(2);
  });

  it("finds the first ayah of the previous surah", () => {
    const inThird = queue && queueAt(queue, 4);
    expect(inThird && surahBoundary(inThird, -1)).toBe(2);
    const inSecondLater = queue && queueAt(queue, 3);
    expect(inSecondLater && surahBoundary(inSecondLater, -1)).toBe(0);
  });

  it("returns null when stepping past the first surah", () => {
    const first = queue && queueAt(queue, 1);
    expect(first && surahBoundary(first, -1)).toBeNull();
  });
});
