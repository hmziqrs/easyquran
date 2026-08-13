import type { DatabaseSync, StatementSync } from "node:sqlite";

import type { QuranQueryRunner, SqlRow, SqlValue } from "$lib/quran/sql";

export function createNodeQueryRunner(database: DatabaseSync): QuranQueryRunner {
  const statements = new Map<string, StatementSync>();
  return {
    all(sql: string, params: readonly SqlValue[] = []): SqlRow[] {
      let statement = statements.get(sql);
      if (!statement) {
        statement = database.prepare(sql);
        statements.set(sql, statement);
      }
      // SAFETY: node:sqlite StatementSync.all() returns one column-name-keyed object per result row; SqlRow is exactly that readonly row shape.
      return statement.all(...params) as SqlRow[];
    },
  };
}
