import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdir, writeFile } from "node:fs/promises";
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

const output = process.env.INDOPAK_DEEP_OUTPUT ?? path.join(root, ".cache/indopak-deep");
const base = process.env.INDOPAK_SPECIMEN_BASE ?? "http://localhost:5391";
const corpus = await loadCorpus(output);
const port = await freePort();
const driver = spawn("/usr/bin/safaridriver", ["-p", String(port)], { stdio: "ignore" });
const session = new Session(`http://127.0.0.1:${port}`);
const report = { engine: "safari", status: "failed", rows: [], errors: [] };
const keySets = new Map();
await mkdir(output, { recursive: true });

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
  await writeFile(
    path.join(output, "safari-corpus-report.json"),
    JSON.stringify(report, null, 2) + "\n",
  );
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
  for (const mode of ["reading", "verse"]) {
    for (let offset = 0; offset < 6236; offset += 256) {
      await session.navigate(
        `${base}/design/indopak?audit=corpus&limit=256&offset=${offset}&mode=${mode}`,
      );
      await session.execute(installErrorRecording);
      await session.waitFor(controlsReady);
      await session.executeAsync(() => document.fonts.ready.then(() => true));
      for (const kind of ["Ring", "Digits"])
        await session.executeAsync(addFont, await diagnosticFont(output, kind));
      const states = SIZES.flatMap((size) =>
        WIDTHS.map((width) => ({ size, width, phone: false })),
      );
      states.push({ size: 48, width: 320, phone: true });
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
        report.rows.push({ mode, offset, ...state, viewport, ...summary, ...overflow });
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
