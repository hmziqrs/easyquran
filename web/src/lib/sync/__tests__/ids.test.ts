import { describe, expect, it } from "vite-plus/test";

import vectors from "../../../../../contracts/uuid-v7.json";
import {
  createUuidV7Generator,
  encodeUuidV7,
  isUuidV7,
  newEntityId,
  uuidV7FromLegacy,
} from "../ids";

function fixedSource(times: readonly number[]) {
  let tick = 0;
  return {
    now: () => times[Math.min(tick++, times.length - 1)] ?? 0,
    random: (length: number) => new Uint8Array(length),
  };
}

function hexBytes(hex: string): Uint8Array {
  return Uint8Array.from({ length: hex.length / 2 }, (_, i) =>
    Number.parseInt(hex.slice(i * 2, i * 2 + 2), 16),
  );
}

describe("UUID v7", () => {
  it.each(vectors.vectors)("matches the shared vector: $note", (vector) => {
    expect(encodeUuidV7(vector.ms, vector.counter, hexBytes(vector.random))).toBe(vector.expected);
  });

  it("mints valid v7 ids", () => {
    expect(isUuidV7(newEntityId())).toBe(true);
    expect(isUuidV7("c1c1c1c1-c1c1-4c1c-8c1c-c1c1c1c1c1c1")).toBe(false);
  });

  it("sorts ids from one ms by creation", () => {
    const next = createUuidV7Generator(fixedSource([5000]));
    const ids = Array.from({ length: 100 }, next);
    expect(ids).toEqual([...ids].sort());
    expect(new Set(ids).size).toBe(100);
  });

  it("sorts ids across ms by creation", () => {
    const next = createUuidV7Generator(fixedSource([5000, 5001, 5002]));
    const ids = [next(), next(), next()];
    expect(ids).toEqual([...ids].sort());
  });

  it("never goes down when the clock moves backwards", () => {
    const next = createUuidV7Generator(fixedSource([9000, 3000, 3000, 9000]));
    const ids = [next(), next(), next(), next()];
    expect(ids).toEqual([...ids].sort());
    expect(new Set(ids).size).toBe(4);
    for (const id of ids) expect(id.startsWith("00000000-2328")).toBe(true);
  });

  it("moves to the next ms when the counter overflows", () => {
    const next = createUuidV7Generator(fixedSource([5000]));
    const ids = Array.from({ length: 4097 }, next);
    expect(ids).toEqual([...ids].sort());
    expect(ids.at(-1)?.slice(0, 13)).toBe("00000000-1389");
  });

  it("re-keys one legacy id to one v7, whoever does it", () => {
    const legacy = "c1c1c1c1-c1c1-4c1c-8c1c-c1c1c1c1c1c1";
    const first = uuidV7FromLegacy(legacy, 1700000000000);
    expect(first).toBe(uuidV7FromLegacy(legacy, 1700000000000));
    expect(isUuidV7(first)).toBe(true);
    expect(first.slice(0, 13)).toBe("018bcfe5-6800");
  });
});
