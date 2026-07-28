/**
 * catalog.ts — (re)build the trimmed web catalog from the full index.
 *
 *   tsx scripts/catalog.ts             # write index.min.json + copy to web/
 *   tsx scripts/catalog.ts --no-web    # write index.min.json only
 *
 * Run this after fetch.ts whenever you want to refresh just the web-facing
 * file without re-downloading anything.
 */

import { INDEX, log, readJson, writeCatalog, type IndexFile } from "./lib";

async function main(): Promise<void> {
  const noWeb = process.argv.slice(2).includes("--no-web");
  log("→ building index.min.json from index.json ...");
  const index = await readJson<IndexFile>(INDEX);
  await writeCatalog(index, { web: !noWeb });
  log(`  ${index.count} translations`);
}

main();
