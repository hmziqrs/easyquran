/**
 * verify.ts — integrity checks for the mirrored translations.
 *
 *   tsx scripts/verify.ts
 *
 * Confirms every translation in index.json has a downloaded SQL dump, that
 * each is valid UTF-8 with a sane verse count, and that recorded checksums
 * and metadata are consistent. Exits non-zero on any hard problem.
 */

import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

import { INDEX, ROOT, log, readJson, type IndexFile } from "./lib";

const MIN_AYAS = 6000; // every full-Quran translation should be near 6236

async function main(): Promise<number> {
  const index = await readJson<IndexFile>(INDEX);
  const trans = index.translations;
  const errors: string[] = [];
  const warnings: string[] = [];

  log(`index.json: ${trans.length} translations (header count=${index.count})`);
  if (trans.length !== index.count) errors.push("index count field != actual entries");

  const ayaDist = new Map<number | "none", number>();
  const missing: string[] = [];
  const badSha: string[] = [];
  const badEnc: string[] = [];
  const short: string[] = [];

  for (const tr of trans) {
    const p = path.join(ROOT, tr.file.sql);
    if (!existsSync(p)) {
      missing.push(tr.id);
      continue;
    }
    const raw = readFileSync(p);
    try {
      new TextDecoder("utf-8", { fatal: true }).decode(raw);
    } catch (e) {
      badEnc.push(`${tr.id}: ${(e as Error).message}`);
    }
    if (createHash("sha256").update(raw).digest("hex") !== tr.file.sha256) badSha.push(tr.id);

    const ac = tr.ayaCount ?? "none";
    ayaDist.set(ac, (ayaDist.get(ac) ?? 0) + 1);
    if (typeof ac === "number" && ac < MIN_AYAS) short.push(`${tr.id} (${ac})`);

    for (const f of ["language", "name", "translator", "direction", "languageCode"] as const) {
      if (!tr[f]) warnings.push(`${tr.id}: missing ${f}`);
    }
  }

  const present = trans.length - missing.length;
  log(`\nfiles present: ${present}/${trans.length}`);
  const dist = [...ayaDist.entries()].sort((a, b) => {
    if (a[0] === "none") return 1;
    if (b[0] === "none") return -1;
    return (a[0] as number) - (b[0] as number);
  });
  log(`aya-count distribution: ${JSON.stringify(Object.fromEntries(dist))}`);
  if (short.length) warnings.push(`translations with < ${MIN_AYAS} ayas: ${short.join(", ")}`);

  for (const [label, items] of [["MISSING", missing], ["BAD ENCODING", badEnc], ["BAD SHA256", badSha]] as const) {
    if (items.length) errors.push(`${label}: ${JSON.stringify(items)}`);
  }

  // group by language
  const byLang = new Map<string, number>();
  for (const tr of trans) byLang.set(tr.language, (byLang.get(tr.language) ?? 0) + 1);
  log(`\nlanguages: ${byLang.size}`);
  for (const [lang, n] of [...byLang.entries()].sort((a, b) => b[1] - a[1])) log(`  ${lang.padEnd(14)} ${n}`);

  const rtl = new Set(trans.filter((tr) => tr.direction === "rtl").map((tr) => tr.language));
  log(`\nrtl languages: ${[...rtl].sort().join(", ")}`);

  log("\nWARNINGS:" + (warnings.length ? "" : " none"));
  for (const w of warnings) log(`  ! ${w}`);
  log("ERRORS:" + (errors.length ? "" : " none"));
  for (const e of errors) log(`  ✗ ${e}`);

  return errors.length ? 1 : 0;
}

main().then((code) => process.exit(code));
