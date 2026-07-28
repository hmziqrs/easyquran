/**
 * fetch.ts — mirror every Quran translation from https://tanzil.net/trans/
 *
 *   tsx scripts/fetch.ts               # download missing files + rebuild indexes
 *   tsx scripts/fetch.ts --force       # re-download every file
 *   tsx scripts/fetch.ts --limit 5     # operate on the first 5 (smoke test)
 *   tsx scripts/fetch.ts --no-web      # don't copy the catalog into web/
 *
 * Writes sql/<id>.sql, index.json (full), and index.min.json (web catalog).
 */

import {
  BASE,
  INDEX,
  buildIndex,
  downloadSql,
  fetchPage,
  log,
  parsePage,
  pPool,
  writeCatalog,
  writeJson,
} from "./lib";

function parseArgs(argv: string[]): { force: boolean; limit: number; noIndex: boolean; noWeb: boolean } {
  const force = argv.includes("--force");
  const noIndex = argv.includes("--no-index");
  const noWeb = argv.includes("--no-web");
  let limit = 0;
  const li = argv.findIndex((a) => a.startsWith("--limit"));
  if (li >= 0) {
    const v = argv[li]!.includes("=") ? argv[li]!.split("=")[1] : argv[li + 1];
    limit = v ? Number(v) : 0;
  }
  return { force, limit, noIndex, noWeb };
}

async function main(): Promise<number> {
  const { force, limit, noIndex, noWeb } = parseArgs(process.argv.slice(2));

  log("→ fetching translation list from tanzil.net/trans/ ...");
  const items = parsePage(await fetchPage());
  log(`  discovered ${items.length} translations`);

  const work = limit ? items.slice(0, limit) : items;
  if (limit) log(`  --limit: processing ${work.length}`);

  log("→ downloading SQL dumps ...");
  const tally = { ok: 0, cached: 0, fail: 0 as number };
  let done = 0;
  await pPool(
    work,
    (it) => downloadSql(it.id, force),
    6,
    (status, it) => {
      tally[status]++;
      done++;
      if (done % 15 === 0 || status === "fail") {
        const flag = status === "fail" ? "✗" : "✓";
        log(`  [${done}/${work.length}] ${flag} ${it.id} — ${status}`);
      }
    },
  );
  log(`  ok=${tally.ok} cached=${tally.cached} failed=${tally.fail}`);

  if (tally.fail) {
    // surface which ones failed by re-scanning
    log("  one or more downloads failed — re-run to retry (idempotent)");
  }

  if (noIndex) return tally.fail ? 1 : 0;

  // build indexes across the FULL discovered set (ignore --limit) so the
  // catalog always reflects every translation tanzil publishes.
  const all = limit ? parsePage(await fetchPage()) : items;
  log("→ building index.json + index.min.json ...");
  const index = buildIndex(all);
  await writeJson(INDEX, index);
  await writeCatalog(index, { web: !noWeb });

  const counts = index.translations.map((t) => t.ayaCount).filter((c): c is number => typeof c === "number");
  if (counts.length) log(`  aya counts: min=${Math.min(...counts)} max=${Math.max(...counts)}`);
  log(`  wrote ${index.count} translations (${BASE}/)`);
  return tally.fail ? 1 : 0;
}

main().then((code) => process.exit(code));
