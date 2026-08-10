import { describe, expect, it } from "vite-plus/test";
import { computeEvictions, type PruneCandidate } from "$lib/workers/opfs-retention";
import { filterSourceFiles, type ActivePointer } from "$lib/workers/opfs-cache";

const DAY_MS = 24 * 60 * 60 * 1000;
const TTL_MS = 30 * DAY_MS;
const CAP_COUNT = 12;
const CAP_BYTES = 128 * 1024 * 1024;
const MB = 1024 * 1024;
const NOW = 1_700_000_000_000;
const CUTOFF = NOW - TTL_MS;

const cand = (id: string, sizeBytes: number): PruneCandidate => ({ id, sizeBytes });
const used = (entries: [string, number][]): Map<string, number> => new Map(entries);
const sized = (entries: [string, number][]): Map<string, number> => new Map(entries);

describe("computeEvictions", () => {
  it("evicts a stale item even when under both caps", () => {
    const evicted = computeEvictions(
      [cand("stale", 1024)],
      used([["stale", CUTOFF - 1]]),
      sized([]),
      NOW,
    );
    expect(evicted).toEqual(["stale"]);
  });

  it("keeps a fresh item sitting exactly on the cutoff when under both caps", () => {
    const evicted = computeEvictions(
      [cand("fresh", 1024)],
      used([["fresh", CUTOFF]]),
      sized([]),
      NOW,
    );
    expect(evicted).toEqual([]);
  });

  it("evicts the single oldest item when the candidate count exceeds the cap", () => {
    const candidates: PruneCandidate[] = [];
    const lastUsed = new Map<string, number>();
    for (let i = 0; i < CAP_COUNT + 1; i++) {
      candidates.push(cand(`c${i}`, 1024));
      lastUsed.set(`c${i}`, CUTOFF + i);
    }
    const evicted = computeEvictions(candidates, lastUsed, sized([]), NOW);
    expect(evicted).toEqual(["c0"]);
  });

  it("keeps every item when the candidate count equals the cap exactly", () => {
    const candidates: PruneCandidate[] = [];
    const lastUsed = new Map<string, number>();
    for (let i = 0; i < CAP_COUNT; i++) {
      candidates.push(cand(`c${i}`, 1024));
      lastUsed.set(`c${i}`, CUTOFF + i);
    }
    const evicted = computeEvictions(candidates, lastUsed, sized([]), NOW);
    expect(evicted).toEqual([]);
  });

  it("evicts the oldest item to bring total bytes under the byte cap", () => {
    const candidates = [cand("older", 70 * MB), cand("newer", 70 * MB)];
    const lastUsed = used([
      ["older", CUTOFF],
      ["newer", CUTOFF + 1],
    ]);
    const evicted = computeEvictions(candidates, lastUsed, sized([]), NOW);
    expect(evicted).toEqual(["older"]);
  });

  it("treats the byte cap as inclusive when the total equals the cap", () => {
    const half = CAP_BYTES / 2;
    const candidates = [cand("a", half), cand("b", half)];
    const lastUsed = used([
      ["a", CUTOFF],
      ["b", CUTOFF + 1],
    ]);
    const evicted = computeEvictions(candidates, lastUsed, sized([]), NOW);
    expect(evicted).toEqual([]);
  });

  it("evicts one item when the total exceeds the byte cap by a single byte", () => {
    const half = Math.floor(CAP_BYTES / 2);
    const candidates = [cand("a", half), cand("b", CAP_BYTES - half + 1)];
    const lastUsed = used([
      ["a", CUTOFF],
      ["b", CUTOFF + 1],
    ]);
    const evicted = computeEvictions(candidates, lastUsed, sized([]), NOW);
    expect(evicted).toEqual(["a"]);
  });

  it("falls back to sizeFor when a candidate reports sizeBytes of zero", () => {
    const evicted = computeEvictions(
      [cand("zero", 0)],
      used([["zero", CUTOFF]]),
      sized([["zero", 200 * MB]]),
      NOW,
    );
    expect(evicted).toEqual(["zero"]);
  });

  it("treats a zero-size candidate with no sizeFor entry as zero bytes", () => {
    const evicted = computeEvictions([cand("zero", 0)], used([["zero", CUTOFF]]), sized([]), NOW);
    expect(evicted).toEqual([]);
  });

  it("prefers the candidate sizeBytes over sizeFor when both are present", () => {
    const evicted = computeEvictions(
      [cand("small", 10)],
      used([["small", CUTOFF]]),
      sized([["small", 200 * MB]]),
      NOW,
    );
    expect(evicted).toEqual([]);
  });

  it("evicts every item when one giant item alone exceeds the byte cap", () => {
    const candidates = [cand("oldest", MB), cand("middle", MB), cand("giant", 200 * MB)];
    const lastUsed = used([
      ["oldest", CUTOFF],
      ["middle", CUTOFF + 1],
      ["giant", CUTOFF + 2],
    ]);
    const evicted = computeEvictions(candidates, lastUsed, sized([]), NOW);
    expect(evicted).toEqual(["oldest", "middle", "giant"]);
  });

  it("evicts never-stamped candidates first via the missing-lastUsed default", () => {
    const candidates = [cand("unknown", 1024), cand("known", 1024)];
    const lastUsed = used([["known", CUTOFF]]);
    const evicted = computeEvictions(candidates, lastUsed, sized([]), NOW);
    expect(evicted).toEqual(["unknown"]);
  });

  it("keeps pinned Arabic artifacts even when stale", () => {
    const evicted = computeEvictions(
      [cand("arabic:2", 1024)],
      used([["arabic:2", CUTOFF - 1]]),
      sized([]),
      NOW,
      ["arabic:2"],
    );
    expect(evicted).toEqual([]);
  });

  it("bounds a synthetic 100-translation cache while preserving the pinned Arabic artifact", () => {
    const arabicIds = ["uthmani"];
    const candidates = [
      ...arabicIds.map((id) => cand(id, 2 * MB)),
      ...Array.from({ length: 100 }, (_, index) => cand(`translation-${index}`, MB)),
    ];
    const lastUsed = new Map(
      candidates.map((candidate, index) => [candidate.id, CUTOFF + index] as const),
    );

    const evicted = computeEvictions(candidates, lastUsed, sized([]), NOW, arabicIds);
    const retainedTranslations = 100 - evicted.length;
    expect(retainedTranslations).toBeLessThanOrEqual(CAP_COUNT);
    expect(retainedTranslations * MB).toBeLessThanOrEqual(CAP_BYTES);
    expect(evicted).not.toContain("uthmani");
  });
});

describe("pointer-aware source filtering drives computeEvictions", () => {
  const pointers = (entries: [string, string][]): Map<string, ActivePointer> =>
    new Map(entries.map(([id, file]) => [id, { sourceId: id, activeFile: file }]));

  it("feeds only pointer-active ids to eviction and never temps or unadopted legacy", () => {
    const files = [
      { tag: "a", name: "a.sqlite" },
      { tag: "a", name: "a.sqlite.tmp" },
      { tag: "b", name: "b.sqlite" },
      { tag: "legacy", name: "legacy.sqlite" },
    ];
    const { sources } = filterSourceFiles({
      pointers: pointers([["a", "a.sqlite"]]),
      files,
    });
    const candidates: PruneCandidate[] = sources.map((s) => ({ id: s.id, sizeBytes: MB }));
    expect(candidates.map((c) => c.id)).toEqual(["a"]);
    expect(candidates.map((c) => c.id)).not.toContain("legacy");

    const evicted = computeEvictions(candidates, used([["a", CUTOFF + 1]]), sized([]), NOW);
    expect(evicted).toEqual([]);
  });

  it("evicts the stale pointer-active source while a temp file is ignored entirely", () => {
    const files = [
      { tag: "stale", name: "stale.sqlite" },
      { tag: "stale", name: "stale.sqlite.tmp" },
    ];
    const { sources } = filterSourceFiles({
      pointers: pointers([["stale", "stale.sqlite"]]),
      files,
    });
    const candidates: PruneCandidate[] = sources.map((s) => ({ id: s.id, sizeBytes: 1024 }));
    expect(candidates).toHaveLength(1);
    const evicted = computeEvictions(candidates, used([["stale", CUTOFF - 1]]), sized([]), NOW);
    expect(evicted).toEqual(["stale"]);
  });
});
