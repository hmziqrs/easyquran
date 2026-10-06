import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";

import { chromium, firefox, webkit } from "playwright";
import { copySpecimens } from "./clipboard-browser-shared.mjs";
import {
  addFont,
  assertSpecimens,
  controlsReady,
  diagnosticFont,
  inspectSpecimens,
  layoutOverflow,
  loadCorpus,
  root,
  SIZES,
  WIDTHS,
} from "./deep-browser-shared.mjs";

const output = process.env.INDOPAK_DEEP_OUTPUT ?? path.join(root, ".cache/indopak-deep");
const base = process.env.INDOPAK_SPECIMEN_BASE ?? "http://localhost:5391";
const verifyCopy = process.env.INDOPAK_CORPUS_COPY === "1";
const destination = process.env.INDOPAK_CORPUS_OUTPUT ?? output;
const selected = (process.env.INDOPAK_DEEP_ENGINES ?? "chromium,webkit").split(",");
const corpus = await loadCorpus(output);
const reports = [];
await mkdir(output, { recursive: true });
await mkdir(destination, { recursive: true });
for (const [name, engine] of Object.entries({ chromium, firefox, webkit })) {
  if (!selected.includes(name)) continue;
  const rows = [];
  const keySets = new Map();
  const errors = [];
  let browser;
  try {
    browser = await engine.launch({ headless: true });
    const page = await browser.newPage({
      viewport: { width: 1100, height: 900 },
      serviceWorkers: "block",
    });
    page.on("pageerror", (error) => errors.push(error.message));
    for (const mode of ["reading", "verse"]) {
      for (let offset = 0; offset < 6236; offset += 256) {
        const response = await page.goto(
          `${base}/design/indopak?audit=corpus&limit=256&offset=${offset}&mode=${mode}`,
        );
        assert.equal(response.status(), 200);
        await page.waitForFunction(controlsReady);
        await page.evaluate(() => document.fonts.ready);
        for (const kind of ["Ring", "Digits"])
          await page.evaluate(addFont, await diagnosticFont(output, kind));
        const states = SIZES.flatMap((size) =>
          WIDTHS.map((width) => ({ size, width, phone: false })),
        );
        states.push({ size: 48, width: 320, phone: true });
        for (const state of states) {
          await page.setViewportSize(
            state.phone ? { width: 390, height: 844 } : { width: 1100, height: 900 },
          );
          await page.getByLabel("Font size").selectOption(String(state.size));
          await page.getByLabel("Run width").selectOption(String(state.width));
          const specimens = await page.evaluate(inspectSpecimens, {
            requireInk: true,
            fontReference: corpus.fontReference,
          });
          assert.equal(specimens.length, Math.min(256, 6236 - offset));
          const summary = assertSpecimens(specimens, corpus, false, true, true);
          let copied = 0;
          if (verifyCopy) {
            const copies = await page.evaluate(copySpecimens);
            assert.equal(copies.length, specimens.length);
            for (const item of copies) {
              assert.equal(item.handled, true, `Copy event ${item.key}`);
              assert.equal(item.copied, corpus.originals[item.key], `Copied source ${item.key}`);
            }
            copied = copies.length;
          }
          const overflow = await page.evaluate(layoutOverflow);
          assert.ok(
            overflow.run_overflow.every((key) => ["12:21", "18:110", "56:23"].includes(key)),
            `Unexpected overflow: ${JSON.stringify(overflow)}`,
          );
          const matrix = `${mode}:${state.size}:${state.width}:${state.phone}`;
          if (!keySets.has(matrix)) keySets.set(matrix, new Set());
          for (const specimen of specimens) {
            assert.ok(!keySets.get(matrix).has(specimen.key));
            keySets.get(matrix).add(specimen.key);
          }
          rows.push({
            mode,
            offset,
            ...state,
            specimens: summary.specimens,
            private_occurrences: summary.private_occurrences,
            ring_checks: summary.ring_checks,
            end_clusters: summary.end_clusters,
            copied_verses: copied,
            ...overflow,
          });
        }
        console.log(`${name}: ${mode} corpus ${Math.min(offset + 256, 6236)}/6236`);
      }
    }
    for (const keys of keySets.values()) assert.equal(keys.size, 6236);
    assert.equal(keySets.size, 32);
    assert.deepEqual(errors, []);
    reports.push({
      engine: name,
      version: browser.version(),
      status: "passed",
      unique_verses_per_matrix: 6236,
      matrices: keySets.size,
      rows,
      errors,
    });
  } catch (error) {
    reports.push({ engine: name, status: "failed", error: String(error.stack), rows, errors });
  } finally {
    await browser?.close();
  }
  await writeFile(
    path.join(destination, `${name}-corpus-report.json`),
    JSON.stringify(reports.at(-1), null, 2) + "\n",
  );
}
console.log(
  JSON.stringify(reports.map(({ rows, ...report }) => ({ ...report, batches: rows.length }))),
);
if (reports.some((report) => report.status !== "passed")) process.exitCode = 1;
