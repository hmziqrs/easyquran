import {
  runOne,
  runQuery,
  type QuranQuery,
  type QuranQueryRunner,
  type SqlValue,
} from "$lib/quran/sql";
import { describe, expect, it } from "vite-plus/test";

const query: QuranQuery<number> = {
  sql: "SELECT value FROM fixture WHERE id = ?",
  decode: (row) => {
    const value = row.value;
    // eslint-disable-next-line anti-slop/no-runtime-typeof -- row.value is a SqlValue runtime union (string | number | null | Uint8Array) read straight from sqlite; typeof is the only discriminator at this decode boundary
    if (typeof value !== "number") throw new Error("invalid fixture value");
    return value;
  },
};

function fixtureRunner(rows: readonly number[]) {
  const calls: { sql: string; params: readonly SqlValue[] }[] = [];
  const runner: QuranQueryRunner = {
    all(sql, params = []) {
      calls.push({ sql, params });
      return rows.map((value) => ({ value }));
    },
  };
  return { calls, runner };
}

describe("shared Quran query execution", () => {
  it("forwards SQL parameters and decodes rows once", () => {
    const fixture = fixtureRunner([7, 9]);
    expect(runQuery(fixture.runner, query, [3])).toEqual([7, 9]);
    expect(fixture.calls).toEqual([{ sql: query.sql, params: [3] }]);
  });

  it("enforces single-row query cardinality", () => {
    expect(runOne(fixtureRunner([7]).runner, query)).toBe(7);
    expect(() => runOne(fixtureRunner([]).runner, query)).toThrow("expected one row");
    expect(() => runOne(fixtureRunner([1, 2]).runner, query)).toThrow("expected one row");
  });
});
