/**
 * catalog.ts — (re)build the trimmed web catalog from the full index.
 *
 *   tsx scripts/catalog.ts             # write index.min.json + copy to web/
 *   tsx scripts/catalog.ts --no-web    # write index.min.json only
 *
 * Requires sqlite/manifest.json (produced by `pnpm build:sqlite`): each
 * catalogue entry's `file` field is widened with the sqlite sizeBytes + sha256.
 * Fails hard if the manifest is missing or any index id is absent from it.
 */

import { existsSync } from "node:fs";
import { INDEX, MANIFEST, log, readJson, writeCatalog, type IndexFile, type SqliteDigest } from "./lib";

async function main(): Promise<void> {
  const noWeb = process.argv.slice(2).includes("--no-web");
  log("→ building index.min.json from index.json ...");
  const index = await readJson<IndexFile>(INDEX);

  if (!existsSync(MANIFEST)) {
    throw new Error(`sqlite/manifest.json missing — run \`pnpm build:sqlite\` first`);
  }
  const manifest = await readJson<SqliteDigest[]>(MANIFEST);
  const sqlite = new Map<string, SqliteDigest>();
  for (const d of manifest) sqlite.set(d.id, d);

  const missing = index.translations.filter((t) => !sqlite.has(t.id)).map((t) => t.id);
  if (missing.length) {
    throw new Error(
      `${missing.length} translation(s) missing from sqlite/manifest.json (run \`pnpm build:sqlite\` first): ${missing.join(", ")}`,
    );
  }

  await writeCatalog(index, { web: !noWeb, sqlite });
  log(`  ${index.count} translations`);
}

main();
