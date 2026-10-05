import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";

import { chromium, firefox, webkit } from "playwright";

import {
  DESKTOP_VIEWPORT,
  PHONE_VIEWPORT,
  SCREENSHOT_KEYS,
  SIZES,
  WIDTHS,
  addFont,
  assertSpecimens,
  auditStyle,
  captureTargets,
  controlsReady,
  diagnosticFont,
  diagnosticKinds,
  domImageName,
  domKinds,
  hideFixedOverlays,
  inspectSpecimens,
  layoutOverflow,
  loadCorpus,
  overlapCandidates,
  paintFailures,
  paintOccurrences,
  root,
  setAuditStyle,
  specimenKeySets,
} from "./deep-browser-shared.mjs";

const output = process.env.INDOPAK_DEEP_OUTPUT ?? path.join(root, ".cache/indopak-deep");
const base = process.env.INDOPAK_SPECIMEN_BASE ?? "http://localhost:5391";
const corpus = await loadCorpus(output);
const reports = [];
const diagnosticsOnly = process.env.INDOPAK_DEEP_DIAGNOSTICS_ONLY === "1";
const engines = (process.env.INDOPAK_DEEP_ENGINES ?? "chromium,firefox,webkit").split(",");
const targetKeys = process.env.INDOPAK_DEEP_KEYS?.split(",");
const flowOnly = process.env.INDOPAK_DEEP_FLOW === "1";
await mkdir(output, { recursive: true });

async function captureDomInk(page, engine, painted) {
  const sets = await page.evaluate(specimenKeySets);
  const targets = captureTargets({ occurrences: corpus.occurrences, painted, targetKeys, ...sets });
  await page.evaluate(hideFixedOverlays);
  const records = [];
  try {
    for (const occurrence of targets.values()) {
      const images = {};
      for (const kind of domKinds(occurrence.code)) {
        await page.evaluate(setAuditStyle, auditStyle(kind, occurrence.key));
        const filename = domImageName(engine, occurrence.key, kind);
        await page
          .locator(`[data-specimen="${occurrence.key}"]`)
          .screenshot({ path: path.join(output, filename) });
        images[kind] = filename;
      }
      records.push({ key: occurrence.key, code: occurrence.code, images });
      if (records.length % 25 === 0)
        console.log(`${engine}: DOM ink ${records.length}/${targets.size}`);
    }
  } finally {
    await page.evaluate(setAuditStyle, "");
  }
  await writeFile(
    path.join(output, `${engine}-dom-images.json`),
    JSON.stringify(records, null, 2) + "\n",
  );
  return records.length;
}

async function inspectDom(page) {
  const summary = assertSpecimens(await page.evaluate(inspectSpecimens), corpus, flowOnly);
  return { ...summary, ...(await page.evaluate(layoutOverflow)) };
}

for (const [name, engine] of Object.entries({ chromium, firefox, webkit })) {
  if (!engines.includes(name)) continue;
  let browser;
  try {
    browser = await engine.launch({ headless: true, timeout: 15000 });
    const page = await browser.newPage({ viewport: DESKTOP_VIEWPORT });
    await page.routeWebSocket("**", (socket) => socket.close());
    const errors = [];
    page.on("pageerror", (error) => errors.push(error.message));
    const matrices = [];
    let painted;
    let domCaptures = 0;
    for (const mode of diagnosticsOnly || flowOnly ? ["reading"] : ["reading", "verse"]) {
      const audit = flowOnly ? "flow" : "all";
      const response = await page.goto(`${base}/design/indopak?audit=${audit}&mode=${mode}`);
      assert.equal(response.status(), 200);
      await page.waitForFunction(controlsReady);
      await page.evaluate(() => document.fonts.ready);
      for (const size of diagnosticsOnly ? [] : SIZES) {
        await page.getByLabel("Font size").selectOption(String(size));
        for (const width of WIDTHS) {
          await page.getByLabel("Run width").selectOption(String(width));
          matrices.push({
            mode: flowOnly ? "flow" : mode,
            size,
            width,
            ...(await inspectDom(page)),
          });
        }
      }
      if (!diagnosticsOnly) console.log(`${name}: full ${mode} matrix passed`);
      if (mode === "reading") {
        for (const kind of diagnosticKinds(corpus.codes)) {
          await page.evaluate(addFont, await diagnosticFont(output, kind));
        }
        painted = await page.evaluate(paintOccurrences, corpus.occurrences);
        await writeFile(
          path.join(output, `${name}-paint.json`),
          JSON.stringify(painted, null, 2) + "\n",
        );
        const failures = paintFailures(painted);
        assert.equal(
          failures.length,
          0,
          `${name}: invalid private ink diagnostics: ${JSON.stringify(failures.slice(0, 3))}`,
        );
        await page.getByLabel("Font size").selectOption("56");
        await page.getByLabel("Run width").selectOption("640");
        domCaptures = await captureDomInk(page, flowOnly ? `${name}-flow` : name, painted);
        console.log(`${name}: ${domCaptures} original DOM ink comparisons captured`);
      }
      if (diagnosticsOnly) continue;
      await page.setViewportSize(PHONE_VIEWPORT);
      await page.getByLabel("Font size").selectOption("48");
      await page.getByLabel("Run width").selectOption("320");
      matrices.push({
        mode: flowOnly ? "flow" : mode,
        size: 48,
        viewport: [PHONE_VIEWPORT.width, PHONE_VIEWPORT.height],
        ...(await inspectDom(page)),
      });
      for (const key of SCREENSHOT_KEYS) {
        if (!(await page.locator(`[data-specimen="${key}"]`).count())) continue;
        await page
          .locator(`[data-specimen="${key}"]`)
          .screenshot({ path: path.join(output, `${name}-${mode}-${key.replace(":", "-")}.png`) });
      }
      await page.setViewportSize(DESKTOP_VIEWPORT);
    }
    assert.deepEqual(errors, []);
    reports.push({
      engine: name,
      version: browser.version(),
      status: diagnosticsOnly ? "diagnostic_checks_passed" : "mechanical_checks_passed",
      matrices,
      raster_occurrences: painted.length,
      invisible_or_clipped: 0,
      diagnostic_unchanged_geometry: true,
      dom_ink_cases: domCaptures,
      overlap_candidates: overlapCandidates(painted),
      overlap_is_review_candidate_not_automatic_semantic_failure: true,
    });
  } catch (error) {
    reports.push({
      engine: name,
      status: "failed_or_unavailable",
      error: error.message.slice(0, 700),
    });
  } finally {
    await browser?.close();
  }
}
await writeFile(
  path.join(output, flowOnly ? "flow-browser-report.json" : "browser-report.json"),
  JSON.stringify(reports, null, 2) + "\n",
);
console.log(
  JSON.stringify(
    reports.map(({ matrices, overlap_candidates, ...rest }) => ({
      ...rest,
      matrices: matrices?.length,
      overlap_candidates: overlap_candidates?.length,
    })),
    null,
    2,
  ),
);
if (reports.some((report) => report.status === "failed_or_unavailable")) process.exitCode = 1;
