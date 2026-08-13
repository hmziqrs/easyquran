import type { Database, SqlValue as WasmSqlValue } from "@sqlite.org/sqlite-wasm";

import type { QuranQueryRunner, SqlRow, SqlValue } from "./sql.ts";

export function createWasmQueryRunner(database: Database): QuranQueryRunner {
  return {
    all(sql: string, params: readonly SqlValue[] = []): SqlRow[] {
      const resultRows: Record<string, WasmSqlValue>[] = [];
      database.exec({
        sql,
        bind: params as WasmSqlValue[],
        rowMode: "object",
        resultRows,
      });
      return resultRows;
    },
  };
}
