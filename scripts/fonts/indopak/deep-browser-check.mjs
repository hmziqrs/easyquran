import assert from "node:assert/strict";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { DatabaseSync } from "node:sqlite";
import { fileURLToPath } from "node:url";

import { chromium, firefox, webkit } from "playwright";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");
const output = process.env.INDOPAK_DEEP_OUTPUT ?? path.join(root, ".cache/indopak-deep");
const base = process.env.INDOPAK_SPECIMEN_BASE ?? "http://localhost:5391";
const occurrences = JSON.parse(await readFile(path.join(output, "occurrences.json"), "utf8"));
const codes = [...new Set(occurrences.map((item) => item.code))].sort((a, b) => a.localeCompare(b));
const database = new DatabaseSync(path.join(root, "db/quran/arabic/quran-indopak.sqlite"), {
  readOnly: true,
});
const rows = database.prepare("SELECT sura, aya, text FROM quran_text").all();
database.close();
const originals = Object.fromEntries(rows.map((row) => [`${row.sura}:${row.aya}`, row.text]));
const expectedPrivate = occurrences.length;
const reports = [];
const diagnosticsOnly = process.env.INDOPAK_DEEP_DIAGNOSTICS_ONLY === "1";
const engines = (process.env.INDOPAK_DEEP_ENGINES ?? "chromium,firefox,webkit").split(",");
const targetKeys = process.env.INDOPAK_DEEP_KEYS?.split(",");
const flowOnly = process.env.INDOPAK_DEEP_FLOW === "1";
await mkdir(output, { recursive: true });

async function captureDomInk(page, engine, painted) {
  const targets = new Map();
  const specimenKeys = new Set(
    await page
      .locator("[data-specimen]")
      .evaluateAll((elements) => elements.map((element) => element.getAttribute("data-specimen"))),
  );
  const inlineKeys = new Set(
    await page
      .locator("[data-specimen]:has(.indopak-body-sign)")
      .evaluateAll((elements) => elements.map((element) => element.getAttribute("data-specimen"))),
  );
  const collisions = new Set(
    painted.filter((item) => item.overlap >= 3).map((item) => `${item.key}:${item.code}`),
  );
  const extraKeys = ["12:1", "16:6", "73:17", "51:54", "79:27", "26:51", "43:15", "4:142"];
  for (const occurrence of occurrences) {
    if (!specimenKeys.has(occurrence.key)) continue;
    const signature = `${occurrence.key}:${occurrence.code}`;
    if (
      ["E003", "E004", "E021"].includes(occurrence.code) ||
      collisions.has(signature) ||
      extraKeys.includes(occurrence.key) ||
      inlineKeys.has(occurrence.key)
    ) {
      if (targetKeys && !targetKeys.includes(occurrence.key)) continue;
      targets.set(signature, occurrence);
    }
  }
  const style = await page.evaluateHandle(() => {
    for (const candidate of document.querySelectorAll("body *")) {
      if (["fixed", "sticky"].includes(getComputedStyle(candidate).position)) {
        candidate.setAttribute("data-audit-hide", "");
      }
    }
    const element = document.createElement("style");
    document.head.append(element);
    return element;
  });
  const records = [];
  try {
    for (const occurrence of targets.values()) {
      const images = {};
      for (const kind of ["Original", "Actual", occurrence.code, `Except ${occurrence.code}`]) {
        await style.evaluate(
          (element, { name, key }) => {
            const selector = `[data-specimen="${key}"]`;
            const ornamentVisibility = /^[A-F0-9]{4}$/.test(name) ? "hidden" : "visible";
            const family = name === "Original" ? "IndoPak Reader Compat" : `IndoPak Audit ${name}`;
            element.textContent = `[data-audit-hide], [data-audit-hide] * {visibility:hidden !important} [data-specimen] {display:none !important} ${selector} {display:block !important;background:white !important;border-color:transparent !important} ${selector} h3, ${selector} .context {visibility:hidden} ${selector} .verse-text.indopak, ${selector} .indopak-end-sign {font-family: "${family}" !important;color:black !important} ${selector} [data-indopak-ornament] {visibility:${ornamentVisibility}} ${selector} .run {background:white !important}`;
          },
          { name: kind, key: occurrence.key },
        );
        const filename = `${engine}-dom-${occurrence.key.replace(":", "-")}-${kind.replaceAll(" ", "-")}.png`;
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
    await style.evaluate((element) => element.remove());
    await style.dispose();
  }
  await writeFile(
    path.join(output, `${engine}-dom-images.json`),
    JSON.stringify(records, null, 2) + "\n",
  );
  return records.length;
}

async function loadDiagnostics(page) {
  for (const kind of ["Actual", "Base", ...codes, ...codes.map((code) => `Except ${code}`)]) {
    const data = await readFile(
      path.join(output, `audit-${kind.toLowerCase().replaceAll(" ", "-")}.woff2`),
    );
    await page.evaluate(
      async ({ bytes, name }) => {
        const face = new FontFace(name, new Uint8Array(bytes));
        document.fonts.add(await face.load());
      },
      { bytes: [...data], name: `IndoPak Audit ${kind}` },
    );
  }
}

async function paint(page) {
  return page.evaluate((items) => {
    const size = 96;
    function pixels(text, family) {
      const canvas = document.createElement("canvas");
      const context = canvas.getContext("2d");
      context.font = `${size}px "IndoPak Reader Compat"`;
      canvas.width = Math.ceil(context.measureText(text).width) + size * 4;
      canvas.height = size * 5;
      context.font = `${size}px "${family}"`;
      context.direction = "rtl";
      context.textAlign = "right";
      context.fillText(text, canvas.width - size * 2, size * 3);
      return {
        width: canvas.width,
        height: canvas.height,
        data: context.getImageData(0, 0, canvas.width, canvas.height).data,
      };
    }
    const negative = pixels("بِسْمِ اللّٰهِ", "IndoPak Audit E021");
    if ([...negative.data].some((value, index) => index % 4 === 3 && value > 96)) {
      throw new Error("Diagnostic font failed transparent-ink control");
    }
    const cache = new Map();
    return items.map((item) => {
      const signature = `${item.code}:${item.excerpt}`;
      let result = cache.get(signature);
      if (!result) {
        const ink = pixels(item.excerpt, `IndoPak Audit ${item.code}`);
        const ordinary = pixels(item.excerpt, `IndoPak Audit Except ${item.code}`);
        const original = pixels(item.excerpt, "IndoPak Audit Actual");
        let painted = 0;
        let overlap = 0;
        let edge = 0;
        let minX = ink.width;
        let maxX = 0;
        let minY = ink.height;
        let maxY = 0;
        for (let y = 0; y < ink.height; y += 1) {
          for (let x = 0; x < ink.width; x += 1) {
            const index = (y * ink.width + x) * 4 + 3;
            if (ink.data[index] <= 96) continue;
            painted += 1;
            if (ordinary.data[index] > 96) overlap += 1;
            if (x === 0 || y === 0 || x === ink.width - 1 || y === ink.height - 1) edge += 1;
            minX = Math.min(minX, x);
            maxX = Math.max(maxX, x);
            minY = Math.min(minY, y);
            maxY = Math.max(maxY, y);
          }
        }
        let outsideMismatch = 0;
        for (let y = 0; y < ink.height; y += 1) {
          for (let x = 0; x < ink.width; x += 1) {
            if (x >= minX - 2 && x <= maxX + 2 && y >= minY - 2 && y <= maxY + 2) continue;
            const index = (y * ink.width + x) * 4 + 3;
            if (Math.abs(original.data[index] - ordinary.data[index]) > 96) outsideMismatch += 1;
          }
        }
        result = {
          painted,
          overlap,
          overlap_fraction: overlap / Math.max(1, painted),
          edge,
          outsideMismatch,
          bounds: [minX, minY, maxX, maxY],
        };
        cache.set(signature, result);
      }
      return { key: item.key, index: item.index, code: item.code, ...result };
    });
  }, occurrences);
}

async function inspectDom(page) {
  const specimens = await page.locator("[data-specimen]").evaluateAll((elements) =>
    elements.map((element) => {
      const key = element.getAttribute("data-specimen");
      const verse = element.querySelector(`[data-verse-key="${key}"] [data-indopak-ayah]`);
      const text = verse.cloneNode(true);
      text.querySelector("[data-indopak-ornament]").remove();
      const privateCount = [...text.textContent].filter((character) =>
        /\p{Co}/u.test(character),
      ).length;
      const finalWord = verse.querySelector(".indopak-final-word");
      const finalBox = finalWord.getBoundingClientRect();
      const container = element.querySelector(".run").getBoundingClientRect();
      const ending = verse.querySelector(".indopak-end-sign");
      const ornament = verse.querySelector(".ayah-ornament");
      const ornamentBox = ornament.getBoundingClientRect();
      const style = getComputedStyle(ornament);
      return {
        key: element.getAttribute("data-specimen"),
        text: text.textContent,
        privateCount,
        final_lines: new Set(
          [...finalWord.getClientRects()].map((rectangle) => Math.round(rectangle.top)),
        ).size,
        final_text: finalWord.textContent,
        final_width: finalBox.width,
        container_width: container.width,
        ornament_width_em: ornamentBox.width / Number.parseFloat(style.fontSize),
        ending_text: ending?.textContent ?? "",
        family: getComputedStyle(verse.parentElement).fontFamily,
      };
    }),
  );
  let count = 0;
  for (const specimen of specimens) {
    assert.equal(specimen.text, originals[specimen.key], `Changed text ${specimen.key}`);
    count += specimen.privateCount;
    assert.ok(
      specimen.ornament_width_em > 0 && specimen.ornament_width_em <= 1.05,
      `Escaped ornament ${specimen.key}`,
    );
    assert.equal(specimen.final_lines, 1, `Split final word ${specimen.key}`);
    assert.ok(/\p{L}/u.test(specimen.final_text), `Missing Arabic word ${specimen.key}`);
    assert.ok(
      specimen.final_width <= specimen.container_width + 1,
      `Final word overflow ${specimen.key}`,
    );
    assert.ok(specimen.family.includes("IndoPak Reader Compat"));
  }
  const expectedKeys = new Set(specimens.map((item) => item.key));
  const expectedCount = occurrences.filter((item) => expectedKeys.has(item.key)).length;
  assert.equal(count, flowOnly ? expectedCount : expectedPrivate);
  if (flowOnly)
    assert.equal(
      expectedKeys.size,
      new Set(occurrences.filter((item) => item.code === "E021").map((item) => item.key)).size,
    );
  return {
    specimens: specimens.length,
    private_occurrences: count,
    end_clusters: specimens.filter((item) => item.ending_text).length,
  };
}

for (const [name, engine] of Object.entries({ chromium, firefox, webkit })) {
  if (!engines.includes(name)) continue;
  let browser;
  try {
    browser = await engine.launch({ headless: true, timeout: 15000 });
    const page = await browser.newPage({ viewport: { width: 1100, height: 900 } });
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
      await page.waitForFunction(() => !document.querySelector("select").disabled);
      await page.evaluate(() => document.fonts.ready);
      for (const size of diagnosticsOnly ? [] : [22, 24, 33, 48, 56]) {
        await page.getByLabel("Font size").selectOption(String(size));
        for (const width of [320, 640, 960]) {
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
        await loadDiagnostics(page);
        painted = await paint(page);
        await writeFile(
          path.join(output, `${name}-paint.json`),
          JSON.stringify(painted, null, 2) + "\n",
        );
        const failures = painted.filter(
          (item) => item.painted === 0 || item.edge > 0 || item.outsideMismatch > 0,
        );
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
      await page.setViewportSize({ width: 390, height: 844 });
      await page.getByLabel("Font size").selectOption("48");
      await page.getByLabel("Run width").selectOption("320");
      matrices.push({
        mode: flowOnly ? "flow" : mode,
        size: 48,
        viewport: [390, 844],
        ...(await inspectDom(page)),
      });
      for (const key of ["17:7", "1:7", "2:101", "16:6", "73:17", "51:54", "26:51", "43:15"]) {
        if (!(await page.locator(`[data-specimen="${key}"]`).count())) continue;
        await page
          .locator(`[data-specimen="${key}"]`)
          .screenshot({ path: path.join(output, `${name}-${mode}-${key.replace(":", "-")}.png`) });
      }
      await page.setViewportSize({ width: 1100, height: 900 });
    }
    assert.deepEqual(errors, []);
    const overlaps = painted.filter((item) => item.overlap >= 8);
    reports.push({
      engine: name,
      version: browser.version(),
      status: diagnosticsOnly ? "diagnostic_checks_passed" : "mechanical_checks_passed",
      matrices,
      raster_occurrences: painted.length,
      invisible_or_clipped: 0,
      diagnostic_unchanged_geometry: true,
      dom_ink_cases: domCaptures,
      overlap_candidates: overlaps.map((item) => ({
        key: item.key,
        index: item.index,
        code: item.code,
        overlap: item.overlap,
        fraction: item.overlap_fraction,
      })),
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
