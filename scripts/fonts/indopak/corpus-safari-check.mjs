import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdir, readFile, rename, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";
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
import { freePort, Session } from "./safari-native-check.mjs";
import { copySpecimens } from "./clipboard-browser-shared.mjs";
import { corpusRowId, corpusStates, resumeCorpus } from "./corpus-checkpoint.mjs";

const output = process.env.INDOPAK_DEEP_OUTPUT ?? path.join(root, ".cache/indopak-deep");
const base = process.env.INDOPAK_SPECIMEN_BASE ?? "http://localhost:5391";
const verifyCopy = process.env.INDOPAK_CORPUS_COPY === "1";
const destination = process.env.INDOPAK_CORPUS_OUTPUT ?? output;
const corpus = await loadCorpus(output);
const states = corpusStates(SIZES, WIDTHS);
const keys = Object.keys(corpus.originals).sort((left, right) => {
  const a = left.split(":").map(Number);
  const b = right.split(":").map(Number);
  return a[0] - b[0] || a[1] - b[1];
});
const font = await readFile(path.join(root, "web/static/fonts/indopak-reader-compat-v4.woff2"));
const rendererSources = {};
for (const filename of [
  "web/src/lib/quran/view/indopak.ts",
  "web/src/routes/(application)/_reader/IndoPakAyah.svelte",
  "web/src/routes/layout.css",
]) {
  const code = await readFile(path.join(root, filename));
  rendererSources[filename] = createHash("sha256").update(code).digest("hex");
}
const scope = {
  schema: 1,
  source_id: "indopak",
  verses: keys.length,
  batch_size: 256,
  modes: ["reading", "verse"],
  states,
  verify_copy: verifyCopy,
  base,
  font_sha256: createHash("sha256").update(font).digest("hex"),
  renderer_sources: rendererSources,
};
const report = { engine: "safari", status: "failed", scope, rows: [], errors: [] };
const completed = new Set();
const resumePath = process.env.INDOPAK_CORPUS_RESUME;
let previousVersion;
if (resumePath) {
  const saved = JSON.parse(await readFile(resumePath, "utf8"));
  const resumed = resumeCorpus(
    saved,
    scope,
    keys,
    corpus.occurrences,
    process.env.INDOPAK_CORPUS_RESUME_LEGACY === "1",
  );
  report.rows = resumed.rows;
  previousVersion = resumed.version;
  report.resume = { path: resumePath, ...resumed, rows: resumed.rows.length };
  for (const row of report.rows) completed.add(corpusRowId(row));
}
const port = await freePort();
const driver = spawn("/usr/bin/safaridriver", ["-p", String(port)], { stdio: "ignore" });
const session = new Session(`http://127.0.0.1:${port}`);
const keySets = new Map();
await mkdir(output, { recursive: true });
await mkdir(destination, { recursive: true });

function installErrorRecording() {
  function record(message) {
    const key = "indopak-corpus-errors";
    const errors = JSON.parse(sessionStorage.getItem(key) ?? "[]");
    errors.push({ message, url: location.href });
    sessionStorage.setItem(key, JSON.stringify(errors));
  }
  window.addEventListener("error", (event) => record(event.message));
  window.addEventListener("unhandledrejection", (event) => record(String(event.reason)));
}

async function save() {
  const target = path.join(destination, "safari-corpus-report.json");
  await writeFile(`${target}.tmp`, JSON.stringify(report, null, 2) + "\n");
  await rename(`${target}.tmp`, target);
}

try {
  for (let attempt = 0; ; attempt += 1) {
    try {
      if ((await session.raw("GET", "/status")).ready) break;
    } catch (error) {
      if (attempt >= 100) throw error;
    }
    await delay(100);
  }
  report.version = await session.start();
  if (previousVersion) {
    for (const name of ["browserVersion", "safari:platformVersion", "safari:platformBuildVersion"])
      assert.equal(report.version[name], previousVersion[name], `Browser changed: ${name}`);
  }
  for (const row of report.rows) {
    const matrix = `${row.mode}:${row.size}:${row.width}:${row.phone}`;
    if (!keySets.has(matrix)) keySets.set(matrix, new Set());
    for (const key of keys.slice(row.offset, row.offset + 256)) keySets.get(matrix).add(key);
  }
  for (const mode of ["reading", "verse"]) {
    for (let offset = 0; offset < 6236; offset += 256) {
      if (states.every((state) => completed.has(corpusRowId({ mode, offset, ...state })))) continue;
      await session.navigate(
        `${base}/design/indopak?audit=corpus&limit=256&offset=${offset}&mode=${mode}`,
      );
      await session.execute(installErrorRecording);
      await session.waitFor(controlsReady);
      await session.executeAsync(() => document.fonts.ready.then(() => true));
      for (const kind of ["Ring", "Digits"])
        await session.executeAsync(addFont, await diagnosticFont(output, kind));
      for (const state of states) {
        const viewport = await session.setViewport(
          state.phone ? { width: 390, height: 844 } : { width: 1100, height: 900 },
        );
        await session.selectByLabel("Font size", String(state.size));
        await session.selectByLabel("Run width", String(state.width));
        const specimens = await session.execute(inspectSpecimens, {
          requireInk: true,
          fontReference: corpus.fontReference,
        });
        assert.equal(specimens.length, Math.min(256, 6236 - offset));
        const summary = assertSpecimens(specimens, corpus, false, true, true);
        let copied = 0;
        if (verifyCopy) {
          const copies = await session.execute(copySpecimens);
          assert.equal(copies.length, specimens.length);
          for (const item of copies) {
            assert.equal(item.handled, true, `Copy event ${item.key}`);
            assert.equal(item.copied, corpus.originals[item.key], `Copied source ${item.key}`);
          }
          copied = copies.length;
        }
        const overflow = await session.execute(layoutOverflow);
        assert.equal(overflow.page_overflow, false);
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
        report.rows.push({
          mode,
          offset,
          ...state,
          viewport,
          ...summary,
          ...overflow,
          copied_verses: copied,
        });
      }
      report.errors.push(
        ...(await session.execute(() => {
          const key = "indopak-corpus-errors";
          const errors = JSON.parse(sessionStorage.getItem(key) ?? "[]");
          sessionStorage.removeItem(key);
          return errors;
        })),
      );
      await save();
      console.log(`safari: ${mode} corpus ${Math.min(offset + 256, 6236)}/6236`);
    }
  }
  for (const keys of keySets.values()) assert.equal(keys.size, 6236);
  assert.equal(keySets.size, 32);
  assert.deepEqual(report.errors, []);
  Object.assign(report, {
    status: "passed",
    unique_verses_per_matrix: 6236,
    matrices: keySets.size,
    error_listener_after_navigation: true,
  });
} catch (error) {
  report.error = String(error.stack ?? error);
  process.exitCode = 1;
} finally {
  await session.stop();
  driver.kill();
  await save();
}
console.log(JSON.stringify({ ...report, rows: report.rows.length }));
