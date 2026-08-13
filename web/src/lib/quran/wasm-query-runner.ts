import type { Database, SqlValue as WasmSqlValue } from "@sqlite.org/sqlite-wasm";

import type { QuranQueryRunner, SqlRow, SqlValue } from "./sql.ts";

export function createWasmQueryRunner(database: Database): QuranQueryRunner {
  return {
    all(sql: string, params: readonly SqlValue[] = []): SqlRow[] {
      // SAFETY: our SqlValue (string|number|null|Uint8Array) is a subset of sqlite-wasm's
      // SqlValue, so the driver accepts every bind param we produce.
      const bind = params as WasmSqlValue[];
      const resultRows: Record<string, SqlValue>[] = [];
      database.exec({
        sql,
        bind,
        rowMode: "object",
        resultRows,
      });
      return resultRows;
    },
  };
}
