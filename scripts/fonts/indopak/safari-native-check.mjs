// Native macOS Safari pass over the deep-audit specimen via the bundled
// /usr/bin/safaridriver (W3C WebDriver). Reuses deep_audit.py inputs and the browser-side
// assertions shared with deep-browser-check.mjs; reports stay separate from Playwright WebKit.
import assert from "node:assert/strict";
import { execFileSync, spawn } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { createServer } from "node:net";
import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";

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
const diagnosticsOnly = process.env.INDOPAK_DEEP_DIAGNOSTICS_ONLY === "1";
const flowOnly = process.env.INDOPAK_DEEP_FLOW === "1";
const skipCritical = process.env.INDOPAK_SAFARI_SKIP_CRITICAL === "1";
const targetKeys = process.env.INDOPAK_DEEP_KEYS?.split(",");
const engine = flowOnly ? "safari-flow" : "safari";
const fontPath = "/fonts/indopak-reader-compat-v3.woff2";
const ELEMENT = "element-6066-11e4-a52e-4f735466cecf";
const CRITICAL_KEYS = [
  "2:101",
  "2:9",
  "2:10",
  "2:99",
  "2:100",
  "2:286",
  "17:7",
  "5:7",
  "6:46",
  "35:2",
  "35:11",
  "1:7",
  "3:4",
  "5:23",
  "97:3",
  "106:4",
  "114:4",
  "6:165",
  "16:6",
  "73:17",
  "51:54",
  "79:27",
  "26:51",
  "43:15",
];
const WRAP_KEYS = ["2:101", "6:165", "16:6", "73:17", "51:54", "79:27", "26:51", "43:15"];
const OVERLAY_STYLE = "[data-audit-hide], [data-audit-hide] * {visibility:hidden !important}";
const captureMismatches = [];
let captureScale = 1;
let tallestViewport = 0;

const corpus = await loadCorpus(output);
const mapping = JSON.parse(
  await readFile(path.join(root, "scripts/fonts/indopak/mapping.json"), "utf8"),
);
await mkdir(output, { recursive: true });

function freePort() {
  return new Promise((resolve, reject) => {
    const server = createServer();
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const { port } = server.address();
      server.close(() => resolve(port));
    });
  });
}

function pngSize(buffer) {
  return [buffer.readUInt32BE(16), buffer.readUInt32BE(20)];
}

class Session {
  constructor(endpoint) {
    this.endpoint = endpoint;
    this.id = null;
  }

  async raw(method, route, body) {
    const init = { method, headers: { "content-type": "application/json" } };
    if (body !== undefined) init.body = JSON.stringify(body);
    const response = await fetch(`${this.endpoint}${route}`, init);
    const json = await response.json();
    if (!response.ok || json.value?.error) {
      throw new Error(`${route}: ${json.value?.error}: ${json.value?.message}`);
    }
    return json.value;
  }

  command(method, route, body) {
    return this.raw(method, `/session/${this.id}${route}`, body);
  }

  async start() {
    const value = await this.raw("POST", "/session", {
      capabilities: {
        alwaysMatch: {
          browserName: "safari",
          pageLoadStrategy: "normal",
          "safari:automaticInspection": false,
          "safari:automaticProfiling": false,
        },
      },
    });
    this.id = value.sessionId;
    await this.command("POST", "/timeouts", { script: 1_800_000, pageLoad: 300_000, implicit: 0 });
    return value.capabilities;
  }

  async stop() {
    if (this.id) await this.raw("DELETE", `/session/${this.id}`).catch(() => {});
    this.id = null;
  }

  execute(fn, ...args) {
    return this.command("POST", "/execute/sync", {
      script: `return (${fn}).apply(null, arguments);`,
      args,
    });
  }

  async executeAsync(fn, ...args) {
    const result = await this.command("POST", "/execute/async", {
      script: `const done = arguments[arguments.length - 1];
        const args = Array.prototype.slice.call(arguments, 0, -1);
        Promise.resolve()
          .then(() => (${fn}).apply(null, args))
          .then((value) => done({ ok: true, value }), (error) => done({ ok: false, error: String((error && error.stack) || error) }));`,
      args,
    });
    if (!result.ok) throw new Error(result.error);
    return result.value;
  }

  async waitFor(fn, args = [], timeout = 120_000) {
    const deadline = Date.now() + timeout;
    while (Date.now() < deadline) {
      if (await this.executeAsync(fn, ...args)) return;
      await delay(100);
    }
    throw new Error(`Timed out waiting for ${fn.name || "condition"}`);
  }

  async find(using, value, from) {
    const route = from ? `/element/${from}/element` : "/element";
    const found = await this.command("POST", route, { using, value });
    return found[ELEMENT];
  }

  async navigate(url) {
    await this.command("POST", "/url", { url });
  }

  async screenshot(element) {
    return Buffer.from(await this.command("GET", `/element/${element}/screenshot`), "base64");
  }

  async setViewport({ width, height }) {
    for (let attempt = 0; attempt < 4; attempt += 1) {
      const [innerWidth, innerHeight] = await this.execute(() => [
        window.innerWidth,
        window.innerHeight,
      ]);
      if (innerWidth === width && innerHeight === height) break;
      const rect = await this.command("GET", "/window/rect");
      await this.command("POST", "/window/rect", {
        width: rect.width + width - innerWidth,
        height: rect.height + height - innerHeight,
      });
    }
    const actual = await this.execute(() => [window.innerWidth, window.innerHeight]);
    return {
      requested: [width, height],
      actual,
      exact: actual[0] === width && actual[1] === height,
    };
  }

  async selectByLabel(label, value) {
    const select = await this.find(
      "xpath",
      `//label[starts-with(normalize-space(.), "${label}")]//select`,
    );
    // Option clicks are not interactable while the select is scrolled out of view.
    await this.execute((target) => target.scrollIntoView({ block: "center" }), {
      [ELEMENT]: select,
    });
    const option = await this.find("css selector", `option[value="${value}"]`, select);
    await this.command("POST", `/element/${option}/click`, {});
    await this.waitFor(selectedValue, [label, String(value)], 10_000);
  }
}

// Browser side.
function selectedValue(label, value) {
  const element = [...document.querySelectorAll("label")].find((candidate) =>
    candidate.textContent.trim().startsWith(label),
  );
  return element?.querySelector("select")?.value === value;
}

// Browser side. Forces style/layout and waits for two frames. WebKit throttles
// requestAnimationFrame for occluded windows, so a timer bounds the wait; geometry reads
// are synchronous and element screenshots paint on demand, so neither needs a frame.
async function framesPainted() {
  await document.fonts.ready;
  document.body.getBoundingClientRect();
  await new Promise((resolve) => {
    const timer = setTimeout(resolve, 250);
    requestAnimationFrame(() =>
      requestAnimationFrame(() => {
        clearTimeout(timer);
        resolve();
      }),
    );
  });
  return true;
}

// Browser side. True once the requested size/width are applied and fonts loaded.
async function settled(size, width) {
  const page = document.querySelector(".specimen-page");
  const sample = document.querySelector("[data-specimen]");
  if (getComputedStyle(page).getPropertyValue("--reader-arabic-size").trim() !== `${size}px`)
    return false;
  if (width && sample?.style.maxWidth !== `${width}px`) return false;
  await document.fonts.ready;
  document.body.getBoundingClientRect();
  return document.fonts.status === "loaded";
}

// Browser side.
function layoutMetrics() {
  const sample = document.querySelector("[data-specimen]");
  return {
    inner: [window.innerWidth, window.innerHeight],
    device_pixel_ratio: window.devicePixelRatio,
    visual_viewport_scale: window.visualViewport?.scale ?? 1,
    sample_width: sample?.getBoundingClientRect().width,
    run_width: sample?.querySelector(".run").getBoundingClientRect().width,
    arabic_size: sample && getComputedStyle(sample.querySelector("[data-indopak-ayah]")).fontSize,
    theme: document.documentElement.dataset.mode,
    visibility: document.visibilityState,
    prefers_dark: window.matchMedia("(prefers-color-scheme: dark)").matches,
  };
}

// Browser side. Fetches the served font bytes and reports face status/glyph coverage.
async function fontEvidence(url) {
  await document.fonts.ready;
  const response = await fetch(url, { cache: "force-cache" });
  const bytes = new Uint8Array(await response.arrayBuffer());
  let binary = "";
  for (let index = 0; index < bytes.length; index += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(index, index + 0x8000));
  }
  const pathname = new URL(url, location.href).pathname;
  const entry = performance
    .getEntriesByType("resource")
    .find((candidate) => new URL(candidate.name).pathname === pathname);
  return {
    status: response.status,
    base64: btoa(binary),
    resource: entry && {
      name: entry.name,
      initiator: entry.initiatorType,
      transfer_size: entry.transferSize,
      encoded_size: entry.encodedBodySize,
      decoded_size: entry.decodedBodySize,
    },
    faces: [...document.fonts]
      .filter((face) => face.family.replaceAll('"', "") === "IndoPak Reader Compat")
      .map((face) => face.status),
    private_glyph_check: document.fonts.check('48px "IndoPak Reader Compat"', ""),
  };
}

// Browser side.
function setTheme(mode) {
  document.documentElement.dataset.mode = mode;
}

// Browser side. Test-only width override on one specimen; null restores the bound style.
function setSpecimenWidth(key, width) {
  const article = document.querySelector(`[data-specimen="${key}"]`);
  if (!article.dataset.auditWidth) article.dataset.auditWidth = article.style.maxWidth;
  article.style.maxWidth = width === null ? article.dataset.auditWidth : `${width}px`;
  if (width === null) delete article.dataset.auditWidth;
}

// Browser side. Narrows one specimen step by step and records which line the final word
// and ornament occupy, so wrapping is observed at its real boundary.
function sweepWrap(key, from, to, step) {
  const article = document.querySelector(`[data-specimen="${key}"]`);
  const saved = article.style.maxWidth;
  const run = article.querySelector(".run");
  const verse = article.querySelector(`[data-verse-key="${key}"] [data-indopak-ayah]`);
  const finalWord = verse.querySelector(".indopak-final-word");
  const ornament = verse.querySelector(".ayah-ornament");
  const lineHeight = Number.parseFloat(getComputedStyle(verse).fontSize);
  function lineIndex(lines, rectangle) {
    const middle = (rectangle.top + rectangle.bottom) / 2;
    return lines.findIndex((top) => Math.abs(top - middle) < lineHeight * 0.75);
  }
  const states = [];
  try {
    for (let width = from; width >= to; width -= step) {
      article.style.maxWidth = `${width}px`;
      const range = document.createRange();
      range.selectNodeContents(verse);
      const lines = [];
      for (const rectangle of range.getClientRects()) {
        if (rectangle.width === 0 || rectangle.height === 0) continue;
        const middle = (rectangle.top + rectangle.bottom) / 2;
        if (!lines.some((top) => Math.abs(top - middle) < lineHeight * 0.75)) lines.push(middle);
      }
      lines.sort((a, b) => a - b);
      const finalRects = [...finalWord.getClientRects()];
      const finalBox = finalWord.getBoundingClientRect();
      const ornamentBox = ornament.getBoundingClientRect();
      const runBox = run.getBoundingClientRect();
      states.push({
        width,
        run_width: runBox.width,
        lines: lines.length,
        final_rects: new Set(finalRects.map((rectangle) => Math.round(rectangle.top))).size,
        final_line: lineIndex(lines, finalBox),
        ornament_line: lineIndex(lines, ornamentBox),
        ornament_inside_run:
          ornamentBox.left >= runBox.left - 1 && ornamentBox.right <= runBox.right + 1,
        final_fits: finalBox.width <= runBox.width + 1,
      });
    }
  } finally {
    article.style.maxWidth = saved;
  }
  return states;
}

function git(...args) {
  return execFileSync("git", args, { cwd: root, encoding: "utf8" }).trim();
}

function environment(capabilities) {
  const command = (file, ...args) => {
    try {
      return execFileSync(file, args, { encoding: "utf8" }).trim();
    } catch (error) {
      return `unavailable: ${error.message.split("\n")[0]}`;
    }
  };
  return {
    commit: git("rev-parse", "HEAD"),
    worktree_dirty: git("status", "--porcelain", "--untracked-files=no") !== "",
    database_id: "quran-indopak",
    origin: base,
    date: new Date().toISOString(),
    browser_name: capabilities.browserName,
    browser_version: capabilities.browserVersion,
    platform_name: capabilities.platformName,
    driver_version: command("/usr/bin/safaridriver", "--version"),
    macos: command("sw_vers", "--productVersion"),
    macos_build: command("sw_vers", "--buildVersion"),
    hardware_model: command("sysctl", "-n", "hw.model"),
    page_zoom: "WebDriver session default (100%); visual_viewport_scale recorded per matrix",
  };
}

async function inspect(session, extra = {}) {
  const specimens = await session.execute(inspectSpecimens);
  const summary = assertSpecimens(specimens, corpus, flowOnly);
  return {
    ...extra,
    ...summary,
    ...(await session.execute(layoutOverflow)),
    measured: await session.execute(layoutMetrics),
  };
}

async function openSpecimen(session, mode) {
  const audit = flowOnly ? "flow" : "all";
  await session.navigate(`${base}/design/indopak?audit=${audit}&mode=${mode}`);
  await session.waitFor(controlsReady);
  await session.executeAsync(framesPainted);
}

async function choose(session, size, width) {
  await session.selectByLabel("Font size", size);
  if (width) await session.selectByLabel("Run width", width);
  await session.waitFor(settled, [size, width ?? null]);
}

// Element screenshot plus a check that Safari returned the whole element box at the
// recorded devicePixelRatio, so viewport clipping cannot pass as a valid capture.
// Safari clips element screenshots to the viewport, so the element is scrolled to the top
// and the viewport grows (height only; width and line breaking stay fixed) when it is taller.
async function shot(session, element, label) {
  let rect = await session.command("GET", `/element/${element}/rect`);
  const [innerWidth, innerHeight] = await session.execute(() => [
    window.innerWidth,
    window.innerHeight,
  ]);
  if (rect.height + 8 > innerHeight) {
    const grown = await session.setViewport({
      width: innerWidth,
      height: Math.ceil(rect.height) + 16,
    });
    assert.equal(grown.actual[0], innerWidth, `${label}: viewport width changed while growing`);
    tallestViewport = Math.max(tallestViewport, grown.actual[1]);
  }
  await session.execute((target) => target.scrollIntoView({ block: "start" }), {
    [ELEMENT]: element,
  });
  rect = await session.command("GET", `/element/${element}/rect`);
  const png = await session.screenshot(element);
  const [width, height] = pngSize(png);
  if (
    Math.abs(width - rect.width * captureScale) > captureScale + 1 ||
    Math.abs(height - rect.height * captureScale) > captureScale + 1
  ) {
    captureMismatches.push({ label, png: [width, height], rect });
  }
  return { png, rect };
}

async function hideOverlays(session) {
  await session.execute(hideFixedOverlays);
  await session.execute(setAuditStyle, OVERLAY_STYLE);
}

async function captureDomInk(session, painted) {
  const sets = await session.execute(specimenKeySets);
  const targets = captureTargets({ occurrences: corpus.occurrences, painted, targetKeys, ...sets });
  await session.execute(hideFixedOverlays);
  const records = [];
  const mismatchesBefore = captureMismatches.length;
  try {
    for (const occurrence of targets.values()) {
      const images = {};
      let rect;
      for (const kind of domKinds(occurrence.code)) {
        await session.execute(setAuditStyle, auditStyle(kind, occurrence.key));
        await session.executeAsync(framesPainted);
        const element = await session.find("css selector", `[data-specimen="${occurrence.key}"]`);
        const capture = await shot(session, element, `${occurrence.key} ${kind}`);
        rect = capture.rect;
        const png = capture.png;
        const filename = domImageName(engine, occurrence.key, kind);
        await writeFile(path.join(output, filename), png);
        images[kind] = filename;
      }
      records.push({
        key: occurrence.key,
        code: occurrence.code,
        images,
        scale: captureScale,
        css_size: [rect.width, rect.height],
      });
      if (records.length % 25 === 0)
        console.log(`${engine}: DOM ink ${records.length}/${targets.size}`);
    }
  } finally {
    await session.execute(setAuditStyle, "");
  }
  await writeFile(
    path.join(output, `${engine}-dom-images.json`),
    JSON.stringify(records, null, 2) + "\n",
  );
  assert.deepEqual(
    captureMismatches.slice(mismatchesBefore),
    [],
    `${engine}: element screenshots do not match element boxes`,
  );
  return records.length;
}

async function screenshotKeys(session, keys, name) {
  const present = new Set((await session.execute(specimenKeySets)).specimenKeys);
  const files = [];
  await hideOverlays(session);
  try {
    for (const key of keys) {
      if (!present.has(key)) continue;
      const element = await session.find("css selector", `[data-specimen="${key}"]`);
      const filename = `${name(key)}.png`;
      await writeFile(path.join(output, filename), (await shot(session, element, filename)).png);
      files.push(filename);
    }
  } finally {
    await session.execute(setAuditStyle, "");
  }
  return {
    present: keys.filter((key) => present.has(key)),
    missing: keys.filter((key) => !present.has(key)),
    files,
  };
}

function criticalKeys() {
  const representatives = mapping.entries.map((entry) => entry.contexts[0].verse_key);
  return [...new Set([...CRITICAL_KEYS, ...representatives])];
}

async function wrapSweep(session, mode, size) {
  const results = [];
  const present = new Set((await session.execute(specimenKeySets)).specimenKeys);
  for (const key of WRAP_KEYS) {
    if (!present.has(key)) continue;
    const states = await session.execute(sweepWrap, key, 960, 200, 2);
    const failures = states.filter(
      (state) =>
        state.final_rects !== 1 ||
        state.final_line !== state.ornament_line ||
        state.final_line < 0 ||
        !state.ornament_inside_run ||
        !state.final_fits,
    );
    const transitions = states.filter(
      (state, index) => index > 0 && state.final_line !== states[index - 1].final_line,
    );
    const captures = [];
    const boundary = transitions[0];
    if (boundary) {
      await hideOverlays(session);
      const element = await session.find("css selector", `[data-specimen="${key}"]`);
      for (const [label, width] of [
        ["before", boundary.width + 2],
        ["at", boundary.width],
        ["after", Math.max(200, boundary.width - 24)],
      ]) {
        await session.execute(setSpecimenWidth, key, width);
        await session.executeAsync(framesPainted);
        const filename = `${engine}-wrap-${mode}-${size}px-${key.replace(":", "-")}-${label}-${width}.png`;
        await writeFile(path.join(output, filename), (await shot(session, element, filename)).png);
        captures.push(filename);
      }
      await session.execute(setSpecimenWidth, key, null);
      await session.execute(setAuditStyle, "");
    }
    results.push({
      key,
      mode,
      size,
      widths: [states[0].width, states.at(-1).width],
      step: 2,
      final_word_transitions: transitions.map((state) => state.width),
      failures,
      captures,
    });
  }
  return results;
}

async function criticalPass(session, mode) {
  const keys = criticalKeys();
  const states = [];
  for (const theme of ["light", "dark"]) {
    await session.execute(setTheme, theme);
    for (const size of [33, 56]) {
      for (const width of [320, 640]) {
        await session.setViewport(DESKTOP_VIEWPORT);
        await choose(session, size, width);
        const matrix = await inspect(session, { mode, theme, size, width });
        const shots = await screenshotKeys(
          session,
          keys,
          (key) =>
            `${engine}-critical-${mode}-${theme}-${size}px-${width}-${key.replace(":", "-")}`,
        );
        states.push({ ...matrix, screenshots: shots.files.length, missing: shots.missing });
      }
    }
  }
  await session.execute(setTheme, "light");
  const sweeps = [];
  for (const size of [33, 56]) {
    await choose(session, size, 640);
    sweeps.push(...(await wrapSweep(session, mode, size)));
  }
  return { states, sweeps };
}

async function main() {
  const port = await freePort();
  const driver = spawn("/usr/bin/safaridriver", ["-p", String(port)], { stdio: "ignore" });
  const session = new Session(`http://127.0.0.1:${port}`);
  const report = { engine, status: "failed_or_unavailable" };
  const matrices = [];
  const critical = [];
  let desktop;
  let painted = [];
  let domCaptures = 0;
  let phone;
  let font;
  try {
    for (let attempt = 0; ; attempt += 1) {
      try {
        if ((await session.raw("GET", "/status")).ready) break;
      } catch (error) {
        if (attempt > 50) throw error;
      }
      await delay(200);
    }
    const capabilities = await session.start();
    report.environment = environment(capabilities);
    desktop = await session.setViewport(DESKTOP_VIEWPORT);
    captureScale = await session.execute(() => window.devicePixelRatio);
    for (const mode of diagnosticsOnly || flowOnly ? ["reading"] : ["reading", "verse"]) {
      await session.setViewport(DESKTOP_VIEWPORT);
      await openSpecimen(session, mode);
      if (!font) {
        const evidence = await session.executeAsync(fontEvidence, fontPath);
        const served = Buffer.from(evidence.base64, "base64");
        const packaged = await readFile(path.join(root, "web/static", fontPath));
        font = {
          url: `${base}${fontPath}`,
          status: evidence.status,
          bytes: served.length,
          packaged_bytes: packaged.length,
          identical_to_package: served.equals(packaged),
          resource: evidence.resource,
          faces: evidence.faces,
          private_glyph_check: evidence.private_glyph_check,
        };
        assert.ok(font.identical_to_package, "Served font differs from packaged font");
        assert.ok(font.faces.includes("loaded"), "IndoPak Reader Compat face did not load");
      }
      for (const size of diagnosticsOnly ? [] : SIZES) {
        for (const width of WIDTHS) {
          await choose(session, size, width);
          matrices.push(await inspect(session, { mode: flowOnly ? "flow" : mode, size, width }));
        }
      }
      if (!diagnosticsOnly) console.log(`${engine}: full ${mode} matrix passed`);
      if (mode === "reading") {
        for (const kind of diagnosticKinds(corpus.codes)) {
          await session.executeAsync(addFont, await diagnosticFont(output, kind));
        }
        painted = await session.executeAsync(paintOccurrences, corpus.occurrences);
        await writeFile(
          path.join(output, `${engine}-paint.json`),
          JSON.stringify(painted, null, 2) + "\n",
        );
        const failures = paintFailures(painted);
        assert.equal(
          failures.length,
          0,
          `${engine}: invalid private ink diagnostics: ${JSON.stringify(failures.slice(0, 3))}`,
        );
        await choose(session, 56, 640);
        domCaptures = await captureDomInk(session, painted);
        console.log(`${engine}: ${domCaptures} original DOM ink comparisons captured`);
        await openSpecimen(session, mode);
      }
      if (diagnosticsOnly) continue;
      phone = await session.setViewport(PHONE_VIEWPORT);
      await choose(session, 48, 320);
      matrices.push(
        await inspect(session, {
          mode: flowOnly ? "flow" : mode,
          size: 48,
          width: 320,
          viewport: phone,
        }),
      );
      await screenshotKeys(
        session,
        SCREENSHOT_KEYS,
        (key) => `${engine}-${mode}-${key.replace(":", "-")}`,
      );
      await session.setViewport(DESKTOP_VIEWPORT);
      if (!flowOnly && !skipCritical) {
        critical.push(await criticalPass(session, mode));
        console.log(`${engine}: critical ${mode} states and wrap sweeps captured`);
      }
    }
    const overflow = matrices.filter(
      (matrix) => matrix.page_overflow || matrix.run_overflow.length > 0,
    );
    const sweepFailures = critical.flatMap((pass) =>
      pass.sweeps.filter((sweep) => sweep.failures.length > 0),
    );
    const candidates = overlapCandidates(painted);
    Object.assign(report, {
      status: diagnosticsOnly ? "diagnostic_checks_passed" : "mechanical_checks_passed",
      viewport: { desktop, phone },
      font,
      matrices,
      layout_overflow_review: overflow.map(
        ({ mode, size, width, page_overflow, run_overflow }) => ({
          mode,
          size,
          width,
          page_overflow,
          run_overflow,
        }),
      ),
      critical,
      wrap_sweep_failures: sweepFailures,
      capture_scale: captureScale,
      tallest_capture_viewport: tallestViewport,
      capture_mismatches: captureMismatches,
      raster_occurrences: painted.length,
      invisible_or_clipped: paintFailures(painted).length,
      dom_ink_cases: domCaptures,
      overlap_candidates: candidates,
      overlap_is_review_candidate_not_automatic_semantic_failure: true,
    });
    if (overflow.length > 0 || sweepFailures.length > 0 || captureMismatches.length > 0)
      report.status = "review_required";
  } catch (error) {
    report.error = String(error.stack ?? error).slice(0, 2000);
    Object.assign(report, {
      partial: true,
      viewport: { desktop, phone },
      font,
      matrices,
      critical,
      raster_occurrences: painted.length,
      dom_ink_cases: domCaptures,
      capture_mismatches: captureMismatches,
    });
  } finally {
    await session.stop();
    driver.kill();
  }
  await writeFile(
    path.join(output, `${engine}-report.json`),
    JSON.stringify(report, null, 2) + "\n",
  );
  const {
    matrices: _matrices,
    critical: _critical,
    overlap_candidates: overlaps,
    ...rest
  } = report;
  console.log(
    JSON.stringify(
      {
        ...rest,
        matrices: matrices.length,
        critical_states: critical.reduce((total, pass) => total + pass.states.length, 0),
        overlap_candidates: overlaps?.length,
      },
      null,
      2,
    ),
  );
  if (report.status !== "mechanical_checks_passed" && report.status !== "diagnostic_checks_passed")
    process.exitCode = 1;
}

await main();
