import { describe, expect, it } from "vite-plus/test";

import rawVectors from "../../../../../contracts/sync-vectors.json";
import {
  canonicalJson,
  compareStamps,
  convergeBookmarks,
  formatStamp,
  isValidStamp,
  mergeBookmark,
  mergeEvent,
  mergeFolder,
  mergeLayout,
  mergeProgress,
  mergeRows,
  mergeSettings,
  mergeThread,
  roundRatio,
  type JsonObject,
} from "../merge";

interface Vector {
  readonly name: string;
  readonly op: string;
  readonly a: JsonObject & readonly JsonObject[];
  readonly b: JsonObject & readonly JsonObject[];
  readonly rows: readonly JsonObject[];
  readonly folders: readonly JsonObject[];
  readonly defaultFolderId: string;
  readonly input: never;
  readonly ascending: readonly string[];
  readonly expected: never;
}

// The shared vector file; each op reads only its own fields.
const vectors: readonly Vector[] = JSON.parse(JSON.stringify(rawVectors.vectors));

const ROW_MERGES = new Map([
  ["thread", mergeThread],
  ["progress", mergeProgress],
  ["bookmark", mergeBookmark],
  ["folder", mergeFolder],
  ["settings", mergeSettings],
  ["layout", mergeLayout],
  // Reading goals are one whole document too: same rule as the home layout.
  ["goals", mergeLayout],
  ["event", mergeEvent],
]);

describe("shared vectors", () => {
  it.each(vectors.map((v) => [v.name, v] as const))("%s", (_name, v) => {
    const merge = ROW_MERGES.get(v.op);
    if (merge !== undefined) {
      expect(canonicalJson(merge(v.a, v.b))).toBe(canonicalJson(v.expected));
    } else if (v.op === "threadSet" || v.op === "folderSet") {
      const domain = v.op === "threadSet" ? "thread" : "folder";
      expect(canonicalJson(mergeRows(domain, v.a, v.b))).toBe(
        canonicalJson(v.expected),
      );
    } else if (v.op === "converge") {
      expect(canonicalJson(convergeBookmarks(v.rows, v.folders, v.defaultFolderId))).toBe(
        canonicalJson(v.expected),
      );
    } else if (v.op === "stampValid") {
      expect(isValidStamp(v.input)).toBe(v.expected);
    } else if (v.op === "stampOrder") {
      for (let i = 1; i < v.ascending.length; i += 1) {
        expect(compareStamps(v.ascending[i - 1] ?? null, v.ascending[i] ?? null)).toBeLessThan(0);
      }
    } else if (v.op === "roundRatio") {
      expect(roundRatio(v.input)).toBe(v.expected);
    } else if (v.op === "canonical") {
      expect(canonicalJson(v.input)).toBe(v.expected);
    } else {
      throw new Error(`unknown op ${v.op}`);
    }
  });
});

class Rng {
  constructor(private state: number) {}
  next(bound: number): number {
    this.state ^= this.state << 13;
    this.state ^= this.state >>> 17;
    this.state ^= this.state << 5;
    return (this.state >>> 0) % bound;
  }
  stamp(): string | null {
    if (this.next(8) === 0) return null;
    return formatStamp(this.next(4), this.next(2), "dev");
  }
  register(range: number): JsonObject {
    return { stamp: this.stamp(), value: this.next(range) };
  }
}

function genThread(r: Rng): JsonObject {
  const keys = ["identity", "cursor", "lifecycle", "future"].filter(() => r.next(3) > 0);
  return { id: "t", ...Object.fromEntries(keys.map((key) => [key, r.register(3)])) };
}

function genFolder(r: Rng): JsonObject {
  const isDefault = r.next(4) === 0;
  const keys = ["name", "position"].filter(() => r.next(3) > 0);
  const row: JsonObject = {
    id: "f",
    isDefault,
    ...Object.fromEntries(keys.map((key) => [key, r.register(3)])),
  };
  return r.next(2) === 0 ? { ...row, deleted: { stamp: r.stamp(), value: r.next(2) === 0 } } : row;
}

function genBookmark(r: Rng): JsonObject {
  return {
    id: "b",
    register: {
      stamp: r.stamp(),
      value: { context: null, deleted: r.next(2) === 0, folderId: "f", key: "1:1" },
    },
  };
}

function genProgress(r: Rng): JsonObject {
  return { surah: 2, furthestAyah: r.next(5) };
}

function genSettings(r: Rng): JsonObject {
  const keys = ["theme", "font"].filter(() => r.next(2) === 0);
  return Object.fromEntries(keys.map((key) => [key, r.register(3)]));
}

describe("merge laws", () => {
  const cases = [
    ["thread", mergeThread, genThread],
    ["folder", mergeFolder, genFolder],
    ["bookmark", mergeBookmark, genBookmark],
    ["progress", mergeProgress, genProgress],
    ["settings", mergeSettings, genSettings],
  ] as const;

  it.each(cases)("%s is commutative, associative, idempotent", (_name, merge, gen) => {
    for (let seed = 1; seed <= 3000; seed += 1) {
      const r = new Rng(Math.imul(seed, 0x9e3779b1) | 1);
      const [a, b, c] = [gen(r), gen(r), gen(r)];
      const text = canonicalJson;
      expect(text(merge(a, b))).toBe(text(merge(b, a)));
      expect(text(merge(merge(a, b), c))).toBe(text(merge(a, merge(b, c))));
      const once = merge(a, a);
      expect(text(merge(once, once))).toBe(text(once));
      const ab = merge(a, b);
      expect(text(merge(ab, b))).toBe(text(ab));
    }
  });

  it("bookmark sets converge in any order after exchange", () => {
    for (let seed = 1; seed <= 800; seed += 1) {
      const r = new Rng(Math.imul(seed, 0x9e3779b1) | 1);
      const versions: JsonObject[] = Array.from({ length: 6 }, () => ({
        id: `b${r.next(3)}`,
        register: {
          stamp: formatStamp(r.next(6), 0, "d"),
          value: {
            context: null,
            deleted: r.next(4) === 0,
            folderId: "f",
            key: `1:${r.next(2)}`,
          },
        },
      }));
      const folders: JsonObject[] = [
        { id: "f", deleted: { stamp: formatStamp(3, 0, "d"), value: r.next(2) === 0 } },
      ];
      const step = (left: readonly JsonObject[], right: readonly JsonObject[]): JsonObject[] =>
        convergeBookmarks(mergeRows("bookmark", left, right), folders, "dflt");
      let x: JsonObject[] = [];
      let y: JsonObject[] = [];
      for (const v of versions) x = step(x, [v]);
      for (const v of [...versions].reverse()) y = step(y, [v]);
      const mergedX = step(x, y);
      expect(canonicalJson(mergedX)).toBe(canonicalJson(step(y, x)));
      expect(canonicalJson(step(mergedX, mergedX))).toBe(canonicalJson(mergedX));
    }
  });
});
