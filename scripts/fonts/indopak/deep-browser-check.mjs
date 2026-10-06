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
  assertRingLayout,
  auditStyle,
  buildRingGrid,
  captureTargets,
  controlsReady,
  diagnosticFont,
  diagnosticKinds,
  fontAdjustmentEvidence,
  domImageName,
  domKinds,
  hideFixedOverlays,
  inspectSpecimens,
  layoutOverflow,
  loadCorpus,
  overlapCandidates,
  paintFailures,
  paintOccurrences,
  paintShapingOccurrences,
  root,
  removeRingGrid,
  ringGridGeometry,
  setRingGridKind,
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
const smoke = process.env.INDOPAK_DEEP_SMOKE === "1";
await mkdir(output, { recursive: true });

async function captureRings(page, engine) {
  const records = [];
  await page.evaluate(hideFixedOverlays);
  try {
    for (const size of SIZES) {
      const record = await page.evaluate(buildRingGrid, size);
      const images = {};
      let original;
      let layoutDrift = 0;
      for (const kind of ["Original", "Actual", "Ring", "Digits"]) {
        await page.evaluate(setRingGridKind, kind);
        await page.evaluate(() => document.fonts.ready);
        const geometry = await page.evaluate(ringGridGeometry);
        if (!original) original = geometry;
        layoutDrift = Math.max(layoutDrift, assertRingLayout(original, geometry));
        const filename = `${engine}-ring-${size}-${kind.toLowerCase()}.png`;
        await page.locator("[data-ring-grid]").screenshot({ path: path.join(output, filename) });
        images[kind] = filename;
      }
      records.push({
        ...record,
        ...original,
        images,
        maximum_layout_drift_css_px: layoutDrift,
        layout_tolerance_css_px: 1 / 64,
      });
    }
  } finally {
    await page.evaluate(removeRingGrid);
  }
  await writeFile(
    path.join(output, `${engine}-ring-images.json`),
    JSON.stringify(records, null, 2) + "\n",
  );
  return records.length * 286;
}

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
        await page.evaluate(async () => {
          document.body.getBoundingClientRect();
          await document.fonts.ready;
        });
        const filename = domImageName(engine, occurrence.key, kind);
        await page
          .locator(`[data-specimen="${occurrence.key}"]`)
          .screenshot({ path: path.join(output, filename) });
        images[kind] = filename;
      }
      await page.evaluate(setAuditStyle, auditStyle("Original", occurrence.key));
      const [specimen] = await page.evaluate(inspectSpecimens, {
        requireInk: true,
        fontReference: corpus.fontReference,
        keys: [occurrence.key],
      });
      const scopes = specimen.occurrences.filter((item) => item.code === occurrence.code);
      assert.equal(
        scopes.length,
        corpus.occurrences.filter(
          (item) => item.key === occurrence.key && item.code === occurrence.code,
        ).length,
      );
      records.push({
        key: occurrence.key,
        code: occurrence.code,
        images,
        occurrences: scopes,
        scale: await page.evaluate(() => window.devicePixelRatio),
        ring_check: specimen.ring_check,
        end_rows: specimen.end_rows,
      });
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
  const summary = assertSpecimens(
    await page.evaluate(inspectSpecimens, {
      requireInk: true,
      fontReference: corpus.fontReference,
    }),
    corpus,
    flowOnly,
  );
  return { ...summary, ...(await page.evaluate(layoutOverflow)) };
}

for (const [name, engine] of Object.entries({ chromium, firefox, webkit })) {
  if (!engines.includes(name)) continue;
  let browser;
  try {
    browser = await engine.launch({ headless: true, timeout: 15000 });
    const page = await browser.newPage({
      viewport: DESKTOP_VIEWPORT,
      deviceScaleFactor: 2,
      serviceWorkers: "block",
    });
    const errors = [];
    page.on("pageerror", (error) => errors.push(error.message));
    const matrices = [];
    let painted;
    let shaping;
    const fontEvidence = [];
    let domCaptures = 0;
    let ringCases = 0;
    for (const mode of diagnosticsOnly || flowOnly ? ["reading"] : ["reading", "verse"]) {
      const audit = flowOnly ? "flow" : "all";
      const response = await page.goto(
        `${base}/design/indopak?audit=${audit}&mode=${mode}&keys=${corpus.captureKeys.join(",")}`,
      );
      assert.equal(response.status(), 200);
      await page.waitForFunction(controlsReady);
      await page.evaluate(() => document.fonts.ready);
      for (const kind of ["Ring", "Digits"])
        await page.evaluate(addFont, await diagnosticFont(output, kind));
      await page.evaluate(addFont, {
        ...(await diagnosticFont(output, "Upstream")),
        sizeAdjust: "100%",
      });
      const adjustment = await page.evaluate(fontAdjustmentEvidence, corpus.fontReference);
      assert.ok(
        adjustment.check && adjustment.samples.every((sample) => sample.honoured),
        `Size-adjust unsupported: ${JSON.stringify(adjustment)}`,
      );
      fontEvidence.push({ mode, ...adjustment });
      let sizes = SIZES;
      let widths = WIDTHS;
      if (smoke) {
        sizes = [33, 56];
        widths = [320, 640];
      }
      for (const size of diagnosticsOnly ? [] : sizes) {
        await page.getByLabel("Font size").selectOption(String(size));
        for (const width of widths) {
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
        ringCases = await captureRings(page, flowOnly ? `${name}-flow` : name);
        const occurrences = corpus.occurrences.filter(
          (item) => !targetKeys || targetKeys.includes(item.key),
        );
        shaping = await page.evaluate(paintShapingOccurrences, occurrences);
        await writeFile(
          path.join(output, `${name}-shaping-only.json`),
          JSON.stringify(shaping, null, 2) + "\n",
        );
        painted = await page.evaluate(paintOccurrences, occurrences);
        await writeFile(
          path.join(output, `${name}-paint.json`),
          JSON.stringify(painted, null, 2) + "\n",
        );
        const failures = paintFailures(shaping);
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
      status: "mechanical_checks_passed",
      scope: {
        diagnostics_only: diagnosticsOnly,
        smoke,
        flow: flowOnly,
        target_keys: targetKeys ?? null,
      },
      font_adjustment: fontEvidence,
      matrices,
      raster_occurrences: painted.length,
      shaping_invisible_or_clipped: 0,
      diagnostic_unchanged_geometry: true,
      dom_ink_cases: domCaptures,
      ring_dom_cases: ringCases,
      ring_dom_approval: "ring_ink_analysis.py",
      shaping_only_overlap_candidates: overlapCandidates(shaping),
      overlap_candidates_source: "dom_ink_analysis.py",
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
  await writeFile(
    path.join(output, `${name}${flowOnly ? "-flow" : ""}-browser-report.json`),
    JSON.stringify(reports.at(-1), null, 2) + "\n",
  );
}
await writeFile(
  path.join(output, flowOnly ? "flow-browser-report.json" : "browser-report.json"),
  JSON.stringify(reports, null, 2) + "\n",
);
console.log(
  JSON.stringify(
    reports.map(({ matrices, shaping_only_overlap_candidates, ...rest }) => ({
      ...rest,
      matrices: matrices?.length,
      shaping_only_overlap_candidates: shaping_only_overlap_candidates?.length,
    })),
    null,
    2,
  ),
);
if (reports.some((report) => report.status === "failed_or_unavailable")) process.exitCode = 1;
